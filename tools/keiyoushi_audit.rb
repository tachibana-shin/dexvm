#!/usr/bin/env ruby
# frozen_string_literal: true

# Downloads Keiyoushi extension APKs and asks dexvm's own DEX resolver which
# external API calls still lack a bytecode or Rust bridge implementation.

require "digest"
require "etc"
require "fileutils"
require "json"
require "net/http"
require "open3"
require "optparse"
require "pathname"
require "thread"
require "time"
require "uri"
require "yaml"

DEFAULT_INDEX = "https://raw.githubusercontent.com/keiyoushi/extensions/repo/index.json"
DEFAULT_CACHE = "target/keiyoushi-audit"
ACTIONABLE = %w[missing-class missing-method static-mismatch unsupported-jni].freeze

Extension = Struct.new(
  :name,
  :package,
  :version,
  :languages,
  :apk_url,
  :apk_name,
  :local_path,
  keyword_init: true
)

class AuditError < StandardError; end

class Reporter
  def initialize
    @mutex = Mutex.new
  end

  def say(message)
    @mutex.synchronize do
      $stderr.puts(message)
      $stderr.flush
    end
  end
end

class Downloader
  REDIRECTS = [301, 302, 303, 307, 308].freeze

  def initialize(refresh:, offline:, verify_cache:, reporter:)
    @refresh = refresh
    @offline = offline
    @verify_cache = verify_cache
    @reporter = reporter
  end

  def fetch(url, destination, expected = nil, validate: true)
    if cache_valid?(destination, expected)
      return file_metadata(destination, url)
    end
    raise AuditError, "offline and not cached: #{url}" if @offline

    FileUtils.mkdir_p(File.dirname(destination))
    part = "#{destination}.part"
    FileUtils.rm_f(part)
    attempts = 0
    begin
      attempts += 1
      download(url, part)
      validate_download!(part) if validate
      File.rename(part, destination)
      file_metadata(destination, url)
    rescue StandardError => e
      FileUtils.rm_f(part)
      retry if attempts < 3
      raise AuditError, "download failed after #{attempts} attempts: #{url}: #{e.message}"
    end
  end

  private

  def cache_valid?(path, expected)
    return false if @refresh || !File.file?(path) || File.size(path).zero?
    return true unless @verify_cache && expected && expected["sha256"]

    expected["size"] == File.size(path) && expected["sha256"] == Digest::SHA256.file(path).hexdigest
  end

  def download(url, destination)
    uri = URI(url)
    6.times do
      raise AuditError, "only HTTPS downloads are allowed: #{uri}" unless uri.is_a?(URI::HTTPS)

      response = Net::HTTP.start(
        uri.host,
        uri.port,
        use_ssl: true,
        open_timeout: 20,
        read_timeout: 120
      ) do |http|
        request = Net::HTTP::Get.new(uri)
        request["User-Agent"] = "dexvm-keiyoushi-audit/1"
        http.request(request) do |res|
          if res.is_a?(Net::HTTPSuccess)
            File.open(destination, "wb") { |file| res.read_body { |chunk| file.write(chunk) } }
          end
        end
      end
      return if response.is_a?(Net::HTTPSuccess)

      if REDIRECTS.include?(response.code.to_i) && response["location"]
        uri = URI.join(uri, response["location"])
        next
      end
      raise AuditError, "HTTP #{response.code} #{response.message}"
    end
    raise AuditError, "too many redirects: #{url}"
  end

  def validate_download!(path)
    header = File.binread(path, 8)
    return if header.start_with?("PK\x03\x04".b, "PK\x05\x06".b, "dex\n".b)

    raise AuditError, "download is not an APK/ZIP/DEX"
  end

  def file_metadata(path, url)
    {
      "url" => url,
      "size" => File.size(path),
      "sha256" => Digest::SHA256.file(path).hexdigest,
      "path" => path
    }
  end
end

class DexcliAudit
  COUNT = /^\s{2}([a-z-]+):\s+(\d+)\s*$/
  GAP = /^\s+\[([^,\]]+), x(\d+)\]\s+(.+?)->([^\(]+)(\(.*)$/
  NE_CLASS = /ne!\s*\(\s*"([^";]+;)"\s*,\s*"/

  def initialize(dexcli)
    @dexcli = dexcli
    @bridge_index = build_bridge_index
  end

  def run(extension, apk_path)
    stdout, stderr, status = Open3.capture3(@dexcli, apk_path, "--api-coverage")
    unless status.success?
      detail = [stderr, stdout].reject(&:empty?).join("\n").lines.last(20).join.strip
      raise AuditError, "dexcli exited #{status.exitstatus}: #{detail}"
    end

    counts = Hash.new(0)
    gaps = []
    stdout.each_line do |line|
      if (match = COUNT.match(line))
        counts[match[1]] = match[2].to_i
      elsif (match = GAP.match(line))
        gaps << {
          "kind" => match[1],
          "uses" => match[2].to_i,
          "class" => match[3],
          "method" => match[4],
          "signature" => match[5],
          "bridge_file" => bridge_file(match[3])
        }
      end
    end
    raise AuditError, "dexcli produced no coverage summary" if counts.empty?

    {
      "name" => extension.name,
      "package" => extension.package,
      "version" => extension.version,
      "languages" => extension.languages,
      "apk" => extension.apk_name,
      "coverage" => counts.sort.to_h,
      "gaps" => gaps
    }
  end

  private

  # Build a class descriptor -> bridge file index by scanning every *.rs
  # under src/vm/native/ for its ne!(...) table registrations. Replaces the
  # previous hardcoded set of files, so new per-class leaf files (java/io/,
  # java/time/, ...) are picked up automatically.
  def build_bridge_index
    index = {}
    Dir.glob(File.join("src/vm/native", "**", "*.rs")).sort.each do |path|
      File.read(path).scan(NE_CLASS).each do |descriptor|
        index[descriptor.first] ||= path
      end
    end
    index
  end

  def bridge_file(class_name)
    @bridge_index[class_name] || fallback_bridge_file(class_name)
  end

  # Classes without native tables (pure shims, e.g. Ljava/math/BigInteger;)
  # have no ne! registration; walk the package chain up to the nearest
  # existing mod.rs so the report still points into the tree.
  def fallback_bridge_file(class_name)
    descriptor = class_name[/^L([^;]+);$/, 1]
    return "src/vm/native/mod.rs" unless descriptor

    parts = descriptor.split("/")
    (1..parts.length).reverse_each do |length|
      candidate = File.join("src/vm/native", *parts.first(length), "mod.rs")
      return candidate if File.file?(candidate)
    end
    "src/vm/native/mod.rs"
  end
end

def load_index(path)
  document = JSON.parse(File.read(path))
  rows = if document.is_a?(Hash)
           document.dig("extensionList", "extensions") || document["extensions"]
         else
           document
         end
  raise AuditError, "unsupported extension index structure" unless rows.is_a?(Array)

  rows.filter_map do |row|
    resources = row["resources"] || {}
    url = resources["apkUrl"] || row["apkUrl"]
    apk_name = row["apk"]
    if !url && apk_name
      url = "https://raw.githubusercontent.com/keiyoushi/extensions/repo/apk/#{apk_name}"
    end
    next unless url

    sources = row["sources"] || []
    languages = sources.filter_map { |source| source["language"] || source["lang"] }.uniq.sort
    languages = [row["lang"]].compact if languages.empty?
    Extension.new(
      name: row["name"] || row["packageName"] || row["pkg"],
      package: row["packageName"] || row["pkg"],
      version: row["versionName"] || row["version"],
      languages: languages,
      apk_url: url,
      apk_name: File.basename(URI(url).path),
      local_path: nil
    )
  end
end

def load_local_apks(directory)
  Dir.glob(File.join(directory, "**", "*.apk")).sort.map do |path|
    name = File.basename(path)
    Extension.new(
      name: name,
      package: name.delete_suffix(".apk"),
      version: nil,
      languages: [],
      apk_url: nil,
      apk_name: name,
      local_path: path
    )
  end
end

def filter_extensions(extensions, options)
  selected = extensions
  if options[:match]
    regex = Regexp.new(options[:match], Regexp::IGNORECASE)
    selected = selected.select do |extension|
      [extension.name, extension.package, extension.apk_name].compact.any? { |value| regex.match?(value) }
    end
  end
  unless options[:languages].empty?
    selected = selected.select { |extension| !(extension.languages & options[:languages]).empty? }
  end
  selected = selected.first(options[:limit]) if options[:limit]
  selected
end

def parallel_map(items, jobs, reporter, label)
  queue = Queue.new
  items.each_with_index { |item, index| queue << [index, item] }
  output = Array.new(items.length)
  errors = []
  mutex = Mutex.new
  completed = 0
  workers = [jobs, items.length].min.times.map do
    Thread.new do
      loop do
        index, item = queue.pop(true)
        begin
          output[index] = yield(item)
        rescue StandardError => e
          mutex.synchronize { errors << [item, e] }
        ensure
          mutex.synchronize do
            completed += 1
            reporter.say("#{label}: #{completed}/#{items.length}") if completed == items.length || (completed % 25).zero?
          end
        end
      rescue ThreadError
        break
      end
    end
  end
  workers.each(&:join)
  [output.compact, errors]
end

def aggregate(results)
  signatures = {}
  totals = Hash.new(0)
  results.each do |result|
    result["coverage"].each { |kind, count| totals[kind] += count }
    result["gaps"].each do |gap|
      key = [gap["kind"], gap["class"], gap["method"], gap["signature"]]
      row = signatures[key] ||= gap.merge("apk_count" => 0, "total_uses" => 0, "examples" => [])
      row["apk_count"] += 1
      row["total_uses"] += gap["uses"]
      row["examples"] << result["apk"] if row["examples"].length < 10
    end
  end
  ranked = signatures.values.sort_by do |row|
    [ACTIONABLE.index(row["kind"]) || ACTIONABLE.length, -row["apk_count"], -row["total_uses"], row["class"], row["method"]]
  end
  [totals.sort.to_h, ranked]
end

def normalized_report(report)
  gap_ids = {}
  normalized_gaps = {}
  report["gaps"].each_with_index do |gap, index|
    id = format("g%04d", index + 1)
    key = [gap["kind"], gap["class"], gap["method"], gap["signature"]]
    gap_ids[key] = id
    normalized_gaps[id] = gap
  end

  extensions = report["extensions"].map do |extension|
    gap_uses = extension["gaps"].to_h do |gap|
      key = [gap["kind"], gap["class"], gap["method"], gap["signature"]]
      [gap_ids.fetch(key), gap["uses"]]
    end
    extension.reject { |key, _value| key == "gaps" }.merge("gap_uses" => gap_uses)
  end

  {
    "schema_version" => 2,
    "generated_at" => report["generated_at"],
    "index" => report["index"],
    "summary" => {
      "selected_count" => report["selected_count"],
      "analyzed_count" => report["analyzed_count"],
      "coverage" => report["coverage"]
    },
    "gaps" => normalized_gaps,
    "extensions" => extensions,
    "download_errors" => report["download_errors"],
    "analysis_errors" => report["analysis_errors"]
  }
end

def write_minified_report(report, path)
  lines = ["schema_version: 1", "counts: [apks, calls]", "missing:"]
  ACTIONABLE.each do |kind|
    gaps = report["gaps"].select { |gap| gap["kind"] == kind }
    next if gaps.empty?

    lines << "  #{kind}:"
    gaps.each do |gap|
      api = "#{gap['class']}->#{gap['method']}#{gap['signature']}"
      lines << "    #{JSON.generate(api)}: [#{gap['apk_count']}, #{gap['total_uses']}]"
    end
  end
  FileUtils.mkdir_p(File.dirname(path))
  File.write(path, lines.join("\n") << "\n")
end

def write_reports(report, yaml_path:, minified_path:, markdown_path:, json_path:, top:)
  normalized = normalized_report(report)
  FileUtils.mkdir_p(File.dirname(yaml_path))
  File.write(yaml_path, YAML.dump(normalized))
  write_minified_report(report, minified_path)
  if json_path
    FileUtils.mkdir_p(File.dirname(json_path))
    File.write(json_path, JSON.generate(normalized) << "\n")
  end

  return unless markdown_path

  lines = [
    "# Keiyoushi APK bridge audit",
    "",
    "Generated: `#{report['generated_at']}`",
    "",
    "- Extensions selected: #{report['selected_count']}",
    "- Extensions analyzed: #{report['analyzed_count']}",
    "- Download failures: #{report['download_errors'].length}",
    "- Analysis failures: #{report['analysis_errors'].length}",
    "- Distinct actionable bridge gaps: #{report['gaps'].count { |gap| ACTIONABLE.include?(gap['kind']) }}",
    "",
    "## Aggregate coverage",
    "",
    "| Resolution | Distinct signatures per APK (summed) |",
    "|---|---:|"
  ]
  report["coverage"].each { |kind, count| lines << "| `#{kind}` | #{count} |" }
  lines.concat([
    "",
    "## Highest-impact gaps",
    "",
    "| Kind | APKs | Calls | Method | Rust bridge | Example APKs |",
    "|---|---:|---:|---|---|---|"
  ])
  report["gaps"].select { |gap| ACTIONABLE.include?(gap["kind"]) }.first(top).each do |gap|
    method = "#{gap['class']}->#{gap['method']}#{gap['signature']}".gsub("|", "\\|")
    examples = gap["examples"].join(", ").gsub("|", "\\|")
    lines << "| `#{gap['kind']}` | #{gap['apk_count']} | #{gap['total_uses']} | `#{method}` | `#{gap['bridge_file']}` | #{examples} |"
  end
  unless report["download_errors"].empty? && report["analysis_errors"].empty?
    lines.concat(["", "## Errors", ""])
    (report["download_errors"] + report["analysis_errors"]).each do |error|
      lines << "- `#{error['apk']}`: #{error['error']}"
    end
  end
  FileUtils.mkdir_p(File.dirname(markdown_path))
  File.write(markdown_path, lines.join("\n") << "\n")
end

options = {
  index: DEFAULT_INDEX,
  cache: DEFAULT_CACHE,
  jobs: [[Etc.nprocessors, 100].max, 120].min,
  languages: [],
  refresh: false,
  refresh_index: false,
  offline: false,
  verify_cache: false,
  download_only: false,
  top: 250
}

parser = OptionParser.new do |opts|
  opts.banner = "Usage: ruby tools/keiyoushi_audit.rb [options]"
  opts.on("--index URL", "Keiyoushi v2/legacy JSON index (default: #{DEFAULT_INDEX})") { |v| options[:index] = v }
  opts.on("--local DIR", "Analyze existing APKs recursively; do not fetch an index") { |v| options[:local] = v }
  opts.on("--cache DIR", "Download/report cache (default: #{DEFAULT_CACHE})") { |v| options[:cache] = v }
  opts.on("--dexcli PATH", "dexcli executable (default: target/debug/dexcli)") { |v| options[:dexcli] = v }
  opts.on("--jobs N", Integer, "Parallel downloads/audits (default: #{options[:jobs]})") { |v| options[:jobs] = v }
  opts.on("--limit N", Integer, "Only process the first N filtered extensions") { |v| options[:limit] = v }
  opts.on("--match REGEX", "Filter by extension name/package/APK") { |v| options[:match] = v }
  opts.on("--language LIST", "Comma-separated source languages") { |v| options[:languages] = v.split(",") }
  opts.on("--refresh", "Redownload APKs") { options[:refresh] = true }
  opts.on("--refresh-index", "Redownload the repository index") { options[:refresh_index] = true }
  opts.on("--verify-cache", "SHA-256 verify every cached APK before reuse") { options[:verify_cache] = true }
  opts.on("--offline", "Use only cached/local data") { options[:offline] = true }
  opts.on("--download-only", "Download APKs without running dexcli") { options[:download_only] = true }
  opts.on("--yaml PATH", "Detailed YAML report path") { |v| options[:yaml] = v }
  opts.on("--minified PATH", "Minimal YAML report path") { |v| options[:minified] = v }
  opts.on("--json PATH", "Also write a compact normalized JSON report") { |v| options[:json] = v }
  opts.on("--markdown PATH", "Markdown report path") { |v| options[:markdown] = v }
  opts.on("--no-markdown", "Do not write the Markdown summary") { options[:no_markdown] = true }
  opts.on("--top N", Integer, "Maximum gaps in Markdown report (default: 250)") { |v| options[:top] = v }
  opts.on("-h", "--help", "Show help") do
    puts opts
    exit
  end
end

begin
  parser.parse!
  raise AuditError, "--jobs must be positive" unless options[:jobs].positive?
  raise AuditError, "--limit must be positive" if options[:limit] && !options[:limit].positive?

  root = Pathname.new(__dir__).join("..").expand_path
  Dir.chdir(root)
  reporter = Reporter.new
  cache = File.expand_path(options[:cache])
  FileUtils.mkdir_p(cache)
  manifest_path = File.join(cache, "manifest.json")
  manifest = File.file?(manifest_path) ? JSON.parse(File.read(manifest_path)) : {}
  downloader = Downloader.new(
    refresh: options[:refresh],
    offline: options[:offline],
    verify_cache: options[:verify_cache],
    reporter: reporter
  )

  extensions = if options[:local]
                 load_local_apks(options[:local])
               else
                 index_path = File.join(cache, "index.json")
                 index_downloader = Downloader.new(
                   refresh: options[:refresh_index],
                   offline: options[:offline],
                   verify_cache: false,
                   reporter: reporter
                 )
                 index_downloader.fetch(options[:index], index_path, nil, validate: false)
                 load_index(index_path)
               end
  extensions = filter_extensions(extensions, options)
  raise AuditError, "no extensions matched" if extensions.empty?
  reporter.say("selected #{extensions.length} extension APKs")

  downloaded, download_errors = if options[:local]
                                  [extensions.map { |extension| [extension, extension.local_path, nil] }, []]
                                else
                                  parallel_map(extensions, options[:jobs], reporter, "download") do |extension|
                                    safe_package = extension.package.to_s.gsub(/[^A-Za-z0-9_.-]/, "_")
                                    path = File.join(cache, "apks", "#{safe_package}--#{extension.apk_name}")
                                    metadata = downloader.fetch(extension.apk_url, path, manifest[extension.apk_url])
                                    [extension, path, metadata]
                                  end
                                end
  downloaded.each do |extension, _path, metadata|
    manifest[extension.apk_url] = metadata if metadata && extension.apk_url
  end
  File.write(manifest_path, JSON.pretty_generate(manifest) << "\n") unless options[:local]

  if options[:download_only]
    reporter.say("downloaded/cached #{downloaded.length}; #{download_errors.length} failures")
    exit(download_errors.empty? ? 0 : 2)
  end

  dexcli = File.expand_path(options[:dexcli] || "target/debug/dexcli")
  raise AuditError, "dexcli not executable: #{dexcli}; run cargo build --features keiyoushi --bin dexcli" unless File.executable?(dexcli)
  auditor = DexcliAudit.new(dexcli)
  results, analysis_errors = parallel_map(downloaded, options[:jobs], reporter, "audit") do |extension, path, _metadata|
    auditor.run(extension, path)
  end
  coverage, gaps = aggregate(results)
  error_rows = lambda do |errors|
    errors.map do |item, error|
      extension = item.is_a?(Array) ? item[0] : item
      { "apk" => extension.apk_name, "error" => error.message }
    end
  end
  report = {
    "generated_at" => Time.now.utc.iso8601,
    "index" => options[:local] ? nil : options[:index],
    "selected_count" => extensions.length,
    "analyzed_count" => results.length,
    "coverage" => coverage,
    "gaps" => gaps,
    "extensions" => results,
    "download_errors" => error_rows.call(download_errors),
    "analysis_errors" => error_rows.call(analysis_errors)
  }
  yaml_path = File.expand_path(options[:yaml] || File.join(cache, "report.yaml"))
  minified_path = File.expand_path(options[:minified] || File.join(cache, "report.min.yaml"))
  json_path = File.expand_path(options[:json]) if options[:json]
  markdown_path = File.expand_path(options[:markdown] || File.join(cache, "report.md")) unless options[:no_markdown]
  write_reports(
    report,
    yaml_path: yaml_path,
    minified_path: minified_path,
    markdown_path: markdown_path,
    json_path: json_path,
    top: options[:top]
  )
  reporter.say("report: #{yaml_path}")
  reporter.say("report: #{minified_path}")
  reporter.say("report: #{json_path}") if json_path
  reporter.say("report: #{markdown_path}") if markdown_path
  exit(download_errors.empty? && analysis_errors.empty? ? 0 : 2)
rescue OptionParser::ParseError, RegexpError, JSON::ParserError, AuditError => e
  warn("error: #{e.message}")
  warn(parser)
  exit 1
end
