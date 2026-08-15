//! org.jsoup host shims on top of dom_query.

use super::*;
use dom_query::{Document, Matcher, NodeId, NodeRef, Selection};

use crate::vm::object::JsoupDocRef;

// ---------------------------------------------------------------------------
// org.jsoup on top of dom_query (mirrors rakuyuki html_element.rs)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// org.jsoup on top of dom_query (mirrors rakuyomi's html_element.rs)
// ---------------------------------------------------------------------------

fn select_selector(selector: &str) -> Option<Matcher> {
    Matcher::new(&normalize_attr_values(&normalize_contains(selector))).ok()
}

/// Quote unquoted attribute values inside `[...]` so dom_query accepts the
/// lenient jsoup syntax (e.g. `a[href*=/truyen/]` -> `a[href*="/truyen/"]`).
fn normalize_attr_values(selector: &str) -> String {
    let mut out = String::with_capacity(selector.len());
    let char_indices: Vec<(usize, char)> = selector.char_indices().collect();
    let mut i = 0;
    while i < char_indices.len() {
        if char_indices[i].1 == '[' {
            let start = i;
            i += 1;
            while i < char_indices.len() && char_indices[i].1 != ']' {
                i += 1;
            }
            let end = i;
            let start_byte = char_indices[start].0;
            let end_byte = if end < char_indices.len() {
                char_indices[end].0
            } else {
                selector.len()
            };
            let frag = &selector[start_byte + 1..end_byte];
            if let Some(eq) = frag.find('=') {
                let value = &frag[eq + 1..];
                let unquoted =
                    !value.trim_start().starts_with('"') && !value.trim_start().starts_with('\'');
                if unquoted {
                    out.push('[');
                    out.push_str(&frag[..eq + 1]);
                    out.push('"');
                    out.push_str(value);
                    out.push('"');
                    out.push(']');
                    i += 1;
                    continue;
                }
            }
            out.push_str(&selector[start_byte..=end_byte.min(selector.len() - 1)]);
            i += 1;
            continue;
        }
        out.push(char_indices[i].1);
        i += 1;
    }
    out
}

/// jsoup `:contains(...)` is not W3C; quote the argument inside `:contains`.
fn normalize_contains(selector: &str) -> String {
    let mut out = String::with_capacity(selector.len());
    let chars: Vec<char> = selector.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i..].starts_with(&[':', 'c', 'o', 'n', 't', 'a', 'i', 'n', 's', '(']) {
            out.push_str(":contains(");
            i += 10;
            let mut inner = String::new();
            let mut depth = 1;
            while i < chars.len() && depth > 0 {
                let c = chars[i];
                if c == '(' {
                    depth += 1;
                } else if c == ')' {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                inner.push(c);
                i += 1;
            }
            let inner_trim = inner.trim();
            if inner_trim.starts_with('"') && inner_trim.ends_with('"') {
                out.push_str(inner_trim);
            } else {
                out.push('"');
                out.push_str(inner_trim);
                out.push('"');
            }
            out.push(')');
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn node_ref_of<'a>(doc: &'a Document, id: NodeId) -> NodeRef<'a> {
    NodeRef::new(id, &doc.tree)
}

/// jsoup element().select: includes the node itself when it matches
/// (mirroring rakuyomi's compatibility layer).
fn select_matches(_doc: &Document, root: NodeRef, selector: &str) -> Vec<NodeId> {
    let Some(matcher) = select_selector(selector) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if matcher.match_element(&root) {
        out.push(root.id);
    }
    out.extend(
        Selection::from(root)
            .select_matcher(&matcher)
            .nodes()
            .iter()
            .map(|n| n.id),
    );
    out
}

/// jsoup Element.text(): descendant text with script/style content dropped,
/// each text node trimmed and joined by a single space.
fn soup_text(node: NodeRef) -> String {
    fn collect(node: NodeRef, out: &mut Vec<String>) {
        if node.is_text() {
            out.push(node.text().to_string());
            return;
        }
        if let Some(tag) = node.node_name() {
            if tag.eq_ignore_ascii_case("script") || tag.eq_ignore_ascii_case("style") {
                return;
            }
        }
        for child in node.children() {
            collect(child, out);
        }
    }
    let mut parts = Vec::new();
    collect(node, &mut parts);
    parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn jsoup_elements(vm: &mut Vm, doc: JsoupDocRef, ids: Vec<NodeId>) -> R {
    alloc(
        vm,
        "Lorg/jsoup/select/Elements;",
        Native::JsoupElements { doc, ids },
    )
}

pub(crate) fn jsoup_parse(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Response { body, request, .. }) = payload(vm, args[0]) else {
        return Err(uoe(vm, "asJsoup: response body missing"));
    };
    let text = String::from_utf8_lossy(body.as_deref().unwrap_or(b"")).into_owned();
    let doc = Document::from(text);
    let base = match payload(vm, *request) {
        Some(Native::Request { url, .. }) => Some(url.clone()),
        _ => None,
    };
    let mut refd = JsoupDocRef::new(doc);
    refd.base = base;
    alloc(vm, "Lorg/jsoup/nodes/Document;", Native::JsoupDoc(refd))
}

/// `Jsoup.parse(html, baseUri)`.
pub(crate) fn jsoup_parse_string(vm: &mut Vm, args: &[JValue]) -> R {
    let text = jstr(vm, args[0])?;
    let base = jstr(vm, args[1])?;
    let mut doc = JsoupDocRef::new(Document::from(text));
    doc.base = Some(base);
    alloc(vm, "Lorg/jsoup/nodes/Document;", Native::JsoupDoc(doc))
}

fn jsoup_parse_string_default(vm: &mut Vm, args: &[JValue]) -> R {
    let base = new_str(vm, "");
    jsoup_parse_string(vm, &[args[0], base])
}

fn jsoup_parse_body_fragment(vm: &mut Vm, args: &[JValue]) -> R {
    let base = if args.len() > 1 {
        args[1]
    } else {
        new_str(vm, "")
    };
    jsoup_parse_string(vm, &[args[0], base])
}

fn jsoup_first_selector_arg(vm: &mut Vm, args: &[JValue]) -> Result<String, NatErr> {
    jstr(vm, args[1]).map_err(|_| npe(vm))
}

fn doc_of(vm: &mut Vm, v: JValue) -> Result<JsoupDocRef, NatErr> {
    let Some(n) = payload(vm, v) else {
        return Err(npe(vm));
    };
    let Some(doc) = doc_of_payload(n) else {
        return Err(npe(vm));
    };
    Ok(doc)
}

pub(crate) fn doc_of_payload(n: &Native) -> Option<JsoupDocRef> {
    match n {
        Native::JsoupDoc(doc) => Some(doc.clone()),
        Native::JsoupElement { doc, .. } | Native::JsoupElements { doc, .. } => Some(doc.clone()),
        _ => None,
    }
}

pub(crate) fn element_id_of(n: &Native) -> Option<NodeId> {
    match n {
        Native::JsoupElement { id, .. } => Some(*id),
        Native::JsoupDoc(doc) => Some(doc.doc.root().id),
        _ => None,
    }
}

pub(crate) fn document_select(vm: &mut Vm, args: &[JValue]) -> R {
    let selector = jsoup_first_selector_arg(vm, args)?;
    document_select_impl(vm, args, selector)
}

fn document_select_impl(vm: &mut Vm, args: &[JValue], selector: String) -> R {
    let doc = doc_of(vm, args[0])?;
    let ids = {
        let d = &*doc.doc;
        let root = d.root();
        select_matches(d, root, &selector)
    };
    jsoup_elements(vm, doc, ids)
}

pub(crate) fn document_text(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc,) = (doc_of(vm, args[0])?,);
    let root = doc.doc.root();
    let text = soup_text(root);
    Ok(vm.alloc_string(&text))
}

pub(crate) fn document_location(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    Ok(vm.alloc_string(doc.base.as_deref().unwrap_or("")))
}

pub(crate) fn document_title(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let title = {
        let root = doc.doc.root();
        select_matches(&doc.doc, root, "title")
            .first()
            .map(|id| soup_text(node_ref_of(&doc.doc, *id)))
            .unwrap_or_default()
    };
    Ok(vm.alloc_string(&title))
}

pub(crate) fn element_select(vm: &mut Vm, args: &[JValue]) -> R {
    let selector = jsoup_first_selector_arg(vm, args)?;
    element_select_impl(vm, args, selector)
}

fn element_select_impl(vm: &mut Vm, args: &[JValue], selector: String) -> R {
    let doc = doc_of(vm, args[0])?;
    let node_id = payload(vm, args[0])
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let ids = {
        let d = &*doc.doc;
        let node = NodeRef::new(node_id, &d.tree);
        select_matches(d, node, &selector)
    };
    jsoup_elements(vm, doc, ids)
}

pub(crate) fn element_select_first(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let node = payload(vm, args[0])
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let selector = jsoup_first_selector_arg(vm, args)?;
    select_first_at(vm, doc, node, &selector)
}

fn select_first_at(vm: &mut Vm, doc: JsoupDocRef, node: NodeId, selector: &str) -> R {
    let out_id = {
        let d = &*doc.doc;
        let node_ref = NodeRef::new(node, &d.tree);
        let Some(matcher) = select_selector(selector) else {
            return Ok(JValue::Null);
        };
        if matcher.match_element(&node_ref) {
            Some(node)
        } else {
            Selection::from(node_ref)
                .select_single_matcher(&matcher)
                .nodes()
                .first()
                .map(|n| n.id)
        }
    };
    match out_id {
        Some(id) => alloc(
            vm,
            "Lorg/jsoup/nodes/Element;",
            Native::JsoupElement { doc, id },
        ),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn element_text(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let payload0 = payload(vm, args[0]);
    let id = payload0.and_then(element_id_of).ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    Ok(vm.alloc_string(&soup_text(node)))
}

pub(crate) fn element_data(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let id = payload(vm, args[0])
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let data = node_ref_of(&doc.doc, id).text().to_string();
    Ok(vm.alloc_string(&data))
}

pub(crate) fn element_val(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let id = payload(vm, args[0])
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let node = node_ref_of(&doc.doc, id);
    let value = if node
        .node_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("textarea"))
    {
        node.text().to_string()
    } else {
        node.attr("value").unwrap_or_default().to_string()
    };
    Ok(vm.alloc_string(&value))
}

pub(crate) fn element_attr(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, name) = (doc_of(vm, args[0])?, jsoup_first_selector_arg(vm, args)?);
    let payload0 = payload(vm, args[0]);
    let id = payload0.and_then(element_id_of).ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    let (abs, name) = match name.strip_prefix("abs:") {
        Some(n) => (true, n.to_string()),
        None => (false, name),
    };
    let mut v = node.attr(&name).unwrap_or_default().to_string();
    if abs {
        v = jsoup_abs_attr(&doc, &v);
    }
    Ok(vm.alloc_string(&v))
}

/// jsoup `abs:` attribute semantics: resolve a relative URL against the
/// document base URI (the response URL).
fn jsoup_abs_attr(doc: &JsoupDocRef, raw: &str) -> String {
    let Some(base) = doc.base.as_ref() else {
        return raw.to_string();
    };
    if raw.starts_with("http://") || raw.starts_with("https://") || raw.is_empty() {
        return raw.to_string();
    }
    if raw.starts_with("//") {
        let scheme = base.split("://").next().unwrap_or("http");
        return format!("{scheme}:{raw}");
    }
    if raw.starts_with('/') {
        let end = base.find("://").map(|i| i + 3).unwrap_or(0);
        let host_end = base[end..].find('/').map(|i| end + i).unwrap_or(base.len());
        return format!("{}{raw}", &base[..host_end]);
    }
    match base.rfind('/') {
        Some(i) if i > base.find("://").unwrap_or(usize::MAX).saturating_add(2) => {
            format!("{}{raw}", &base[..=i])
        }
        _ => format!("{base}/{raw}"),
    }
}

pub(crate) fn element_children(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let payload0 = payload(vm, args[0]);
    let id = payload0.and_then(element_id_of).ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    let ids: Vec<dom_query::NodeId> = node.children().iter().map(|c| c.id).collect();
    jsoup_elements(vm, doc, ids)
}

pub(crate) fn element_parent(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let payload0 = payload(vm, args[0]);
    let id = payload0.and_then(element_id_of).ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    match node.parent() {
        Some(p) => alloc(
            vm,
            "Lorg/jsoup/nodes/Element;",
            Native::JsoupElement {
                doc: doc.clone(),
                id: p.id,
            },
        ),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn element_parents(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let id = payload(vm, args[0])
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let ids = node_ref_of(&doc.doc, id)
        .ancestors(None)
        .into_iter()
        .filter(|node| node.is_element())
        .map(|node| node.id)
        .collect();
    jsoup_elements(vm, doc, ids)
}

pub(crate) fn element_previous_sibling(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let id = payload(vm, args[0])
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let previous = node_ref_of(&doc.doc, id)
        .prev_element_sibling()
        .map(|node| node.id);
    match previous {
        Some(id) => alloc(
            vm,
            "Lorg/jsoup/nodes/Element;",
            Native::JsoupElement { doc, id },
        ),
        None => Ok(JValue::Null),
    }
}

fn element_next_sibling(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let id = payload(vm, args[0])
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let next = node_ref_of(&doc.doc, id)
        .next_element_sibling()
        .map(|node| node.id);
    match next {
        Some(id) => alloc(
            vm,
            "Lorg/jsoup/nodes/Element;",
            Native::JsoupElement { doc, id },
        ),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn element_abs_url(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, name) = (doc_of(vm, args[0])?, jsoup_first_selector_arg(vm, args)?);
    let payload0 = payload(vm, args[0]);
    let id = payload0.and_then(element_id_of).ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    let v = node.attr(&name).unwrap_or_default().to_string();
    Ok(vm.alloc_string(&jsoup_abs_attr(&doc, &v)))
}

pub(crate) fn element_has_attr(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, name) = (doc_of(vm, args[0])?, jsoup_first_selector_arg(vm, args)?);
    let payload0 = payload(vm, args[0]);
    let id = payload0.and_then(element_id_of).ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    Ok(JValue::Int(i32::from(node.has_attr(&name))))
}

pub(crate) fn element_id_attr(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let payload0 = payload(vm, args[0]);
    let id = payload0.and_then(element_id_of).ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    Ok(vm.alloc_string(&node.attr("id").unwrap_or_default()))
}

pub(crate) fn element_tag_name(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let payload0 = payload(vm, args[0]);
    let id = payload0.and_then(element_id_of).ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    let tag = node.node_name().unwrap_or_default().to_lowercase();
    Ok(vm.alloc_string(&tag))
}

pub(crate) fn element_own_text(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let payload0 = payload(vm, args[0]);
    let id = payload0.and_then(element_id_of).ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    Ok(vm.alloc_string(node.immediate_text().as_ref()))
}

pub(crate) fn element_html(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let payload0 = payload(vm, args[0]);
    let id = payload0.and_then(element_id_of).ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    Ok(vm.alloc_string(node.inner_html().as_ref()))
}

pub(crate) fn element_outer_html(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let payload0 = payload(vm, args[0]);
    let id = payload0.and_then(element_id_of).ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    Ok(vm.alloc_string(node.html().as_ref()))
}

pub(crate) fn elements_first(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = match payload(vm, args[0]) {
        Some(Native::JsoupElements { doc, ids }) => (doc.clone(), ids.clone()),
        _ => return Err(npe(vm)),
    };
    match ids.first() {
        Some(id) => alloc(
            vm,
            "Lorg/jsoup/nodes/Element;",
            Native::JsoupElement { doc, id: *id },
        ),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn elements_get(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = match payload(vm, args[0]) {
        Some(Native::JsoupElements { doc, ids }) => (doc.clone(), ids.clone()),
        _ => return Err(npe(vm)),
    };
    let idx = int_of(vm, args[1]);
    match ids.get(idx as usize) {
        Some(id) => alloc(
            vm,
            "Lorg/jsoup/nodes/Element;",
            Native::JsoupElement { doc, id: *id },
        ),
        None => Err(aioobe(vm, idx, ids.len() as i32)),
    }
}

pub(crate) fn elements_size(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::JsoupElements { ids, .. }) => Ok(JValue::Int(ids.len() as i32)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn elements_is_empty(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::JsoupElements { ids, .. }) => Ok(JValue::Int(i32::from(ids.is_empty()))),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn elements_text(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = match payload(vm, args[0]) {
        Some(Native::JsoupElements { doc, ids }) => (doc.clone(), ids.clone()),
        _ => return Err(npe(vm)),
    };
    let d = &*doc.doc;
    let mut parts = Vec::new();
    for id in &ids {
        parts.push(soup_text(node_ref_of(d, *id)));
    }
    let s = parts
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(vm.alloc_string(&s))
}

pub(crate) fn elements_last(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = match payload(vm, args[0]) {
        Some(Native::JsoupElements { doc, ids }) => (doc.clone(), ids.clone()),
        _ => return Err(npe(vm)),
    };
    match ids.last() {
        Some(id) => alloc(
            vm,
            "Lorg/jsoup/nodes/Element;",
            Native::JsoupElement { doc, id: *id },
        ),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn elements_each_text(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = match payload(vm, args[0]) {
        Some(Native::JsoupElements { doc, ids }) => (doc.clone(), ids.clone()),
        _ => return Err(npe(vm)),
    };
    let d = &*doc.doc;
    let items = ids
        .iter()
        .map(|id| vm.alloc_string(&soup_text(node_ref_of(d, *id))))
        .collect();
    list_alloc(vm, items)
}

pub(crate) fn elements_attr(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = match payload(vm, args[0]) {
        Some(Native::JsoupElements { doc, ids }) => (doc.clone(), ids.clone()),
        _ => return Err(npe(vm)),
    };
    let name = jsoup_first_selector_arg(vm, args)?;
    let (abs, name) = match name.strip_prefix("abs:") {
        Some(n) => (true, n.to_string()),
        None => (false, name),
    };
    let d = &*doc.doc;
    let mut value = String::new();
    for id in &ids {
        if let Some(v) = node_ref_of(d, *id).attr(&name) {
            value = v.to_string();
            break;
        }
    }
    if abs {
        value = jsoup_abs_attr(&doc, &value);
    }
    Ok(vm.alloc_string(&value))
}

pub(crate) fn elements_each_attr(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = match payload(vm, args[0]) {
        Some(Native::JsoupElements { doc, ids }) => (doc.clone(), ids.clone()),
        _ => return Err(npe(vm)),
    };
    let name = jsoup_first_selector_arg(vm, args)?;
    let (abs, name) = match name.strip_prefix("abs:") {
        Some(n) => (true, n.to_string()),
        None => (false, name),
    };
    let d = &*doc.doc;
    let items = ids
        .iter()
        .map(|id| {
            let mut v = node_ref_of(d, *id)
                .attr(&name)
                .unwrap_or_default()
                .to_string();
            if abs {
                v = jsoup_abs_attr(&doc, &v);
            }
            vm.alloc_string(&v)
        })
        .collect();
    list_alloc(vm, items)
}

pub(crate) fn elements_select(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = match payload(vm, args[0]) {
        Some(Native::JsoupElements { doc, ids }) => (doc.clone(), ids.clone()),
        _ => return Err(npe(vm)),
    };
    let selector = jsoup_first_selector_arg(vm, args)?;
    let d = &*doc.doc;
    let mut out = Vec::new();
    for id in &ids {
        out.extend(select_matches(d, node_ref_of(d, *id), &selector));
    }
    jsoup_elements(vm, doc, out)
}

// ---------------------------------------------------------------------------
// audit-gap natives: parser/entities, evaluators, text nodes, attributes,
// element & elements mutations
// ---------------------------------------------------------------------------

fn elements_payload(vm: &mut Vm, v: JValue) -> Result<(JsoupDocRef, Vec<NodeId>), NatErr> {
    match payload(vm, v) {
        Some(Native::JsoupElements { doc, ids }) => Ok((doc.clone(), ids.clone())),
        _ => Err(npe(vm)),
    }
}

fn element_payload(vm: &mut Vm, v: JValue) -> Result<(JsoupDocRef, NodeId), NatErr> {
    let doc = doc_of(vm, v)?;
    let id = payload(vm, v)
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    Ok((doc, id))
}

fn obj_class_desc(vm: &Vm, v: JValue) -> Option<String> {
    let id = match v {
        JValue::Obj(id) => id,
        _ => return None,
    };
    let o = vm.arena.get(id)?;
    let cl = vm.classes.get(o.class as usize)?;
    Some(vm.str_of(cl.descriptor).to_string())
}

fn eval_selector_of(vm: &mut Vm, v: JValue) -> Result<String, NatErr> {
    let Some(Native::Str(s)) = payload(vm, v) else {
        return Err(iae(vm, "not an Evaluator"));
    };
    let desc = obj_class_desc(vm, v);
    if desc.as_deref().is_some_and(|d| d.ends_with("$Class")) {
        Ok(s.split_whitespace()
            .map(|c| format!(".{c}"))
            .collect::<Vec<_>>()
            .join(""))
    } else if desc.as_deref().is_some_and(|d| d.ends_with("$Id")) {
        Ok(format!("#{s}"))
    } else {
        Ok(s.clone())
    }
}

fn textnode_alloc(vm: &mut Vm, text: &str) -> R {
    alloc(
        vm,
        "Lorg/jsoup/nodes/TextNode;",
        Native::Str(text.to_string()),
    )
}

fn attribute_alloc(vm: &mut Vm, key: &str, value: &str) -> R {
    let k = new_str(vm, key);
    let v = new_str(vm, value);
    alloc(vm, "Lorg/jsoup/nodes/Attribute;", Native::Pair(k, v))
}

fn fragment_first_element(doc: &Document) -> Option<NodeId> {
    doc.html_root().element_children().first().map(|n| n.id)
}

fn whole_text_of(node: NodeRef) -> String {
    fn collect(node: NodeRef, out: &mut String) {
        if node.is_text() {
            out.push_str(node.text().as_ref());
            return;
        }
        for child in node.children() {
            collect(child, out);
        }
    }
    let mut s = String::new();
    collect(node, &mut s);
    s
}

fn elements_matching(
    vm: &mut Vm,
    doc: JsoupDocRef,
    root_id: NodeId,
    include_self: bool,
    mut pred: impl FnMut(&NodeRef<'_>) -> bool,
) -> R {
    let d = &*doc.doc;
    let root = node_ref_of(d, root_id);
    let mut ids = Vec::new();
    if include_self && root.is_element() && pred(&root) {
        ids.push(root.id);
    }
    for node in root.descendants() {
        if node.is_element() && pred(&node) {
            ids.push(node.id);
        }
    }
    jsoup_elements(vm, doc, ids)
}

fn unescape_entities(s: &str, strict: bool) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'&' {
            let c = s[i..].chars().next().unwrap();
            out.push(c);
            i += c.len_utf8();
            continue;
        }
        let start = i + 1;
        let mut j = start;
        if j < b.len() && b[j] == b'#' {
            j += 1;
            while j < b.len() && b[j].is_ascii_hexdigit() {
                j += 1;
            }
        } else {
            while j < b.len() && b[j].is_ascii_alphanumeric() {
                j += 1;
            }
        }
        let terminated = j < b.len() && b[j] == b';';
        let name = &s[start..j];
        let lenient = !strict && j > start;
        let decoded = if terminated || lenient {
            named_entity(name)
        } else {
            None
        };
        match decoded {
            Some(repl) => {
                out.push_str(&repl);
                i = j + usize::from(terminated);
            }
            None => {
                out.push('&');
                i += 1;
            }
        }
    }
    out
}

fn named_entity(name: &str) -> Option<String> {
    if let Some(digits) = name.strip_prefix('#') {
        let code = if let Some(hex) = digits
            .strip_prefix('x')
            .or_else(|| digits.strip_prefix('X'))
        {
            u32::from_str_radix(hex, 16)
        } else {
            digits.parse::<u32>()
        };
        return code.ok().and_then(char::from_u32).map(|c| c.to_string());
    }
    let s = match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{a0}",
        "copy" => "\u{a9}",
        "reg" => "\u{ae}",
        "trade" => "\u{2122}",
        "hellip" => "\u{2026}",
        "mdash" => "\u{2014}",
        "ndash" => "\u{2013}",
        "lsquo" => "\u{2018}",
        "rsquo" => "\u{2019}",
        "ldquo" => "\u{201c}",
        "rdquo" => "\u{201d}",
        "sbquo" => "\u{201a}",
        "bdquo" => "\u{201e}",
        "laquo" => "\u{ab}",
        "raquo" => "\u{bb}",
        "bull" => "\u{2022}",
        "middot" => "\u{b7}",
        "dagger" => "\u{2020}",
        "Dagger" => "\u{2021}",
        "permil" => "\u{2030}",
        "prime" => "\u{2032}",
        "Prime" => "\u{2033}",
        "larr" => "\u{2190}",
        "uarr" => "\u{2191}",
        "rarr" => "\u{2192}",
        "darr" => "\u{2193}",
        "harr" => "\u{2194}",
        "prop" => "\u{221d}",
        "infin" => "\u{221e}",
        "sum" => "\u{2211}",
        "prod" => "\u{220f}",
        "radic" => "\u{221a}",
        "le" => "\u{2264}",
        "ge" => "\u{2265}",
        "ne" => "\u{2260}",
        "asymp" => "\u{2248}",
        "equiv" => "\u{2261}",
        "sub" => "\u{2282}",
        "sup" => "\u{2283}",
        "nsub" => "\u{2284}",
        "sube" => "\u{2286}",
        "supe" => "\u{2287}",
        "oplus" => "\u{2295}",
        "otimes" => "\u{2297}",
        "cap" => "\u{2229}",
        "cup" => "\u{222a}",
        "in" => "\u{2208}",
        "notin" => "\u{2209}",
        "empty" => "\u{2205}",
        "exists" => "\u{2203}",
        "forall" => "\u{2200}",
        "and" => "\u{2227}",
        "or" => "\u{2228}",
        "not" => "\u{ac}",
        "plusmn" => "\u{b1}",
        "times" => "\u{d7}",
        "divide" => "\u{f7}",
        "deg" => "\u{b0}",
        "para" => "\u{b6}",
        "sect" => "\u{a7}",
        "micro" => "\u{b5}",
        "cent" => "\u{a2}",
        "pound" => "\u{a3}",
        "euro" => "\u{20ac}",
        "yen" => "\u{a5}",
        "curren" => "\u{a4}",
        "brvbar" => "\u{a6}",
        "frac12" => "\u{bd}",
        "frac14" => "\u{bc}",
        "frac34" => "\u{be}",
        "sup2" => "\u{b2}",
        "sup3" => "\u{b3}",
        "sup1" => "\u{b9}",
        "shy" => "\u{ad}",
        "auml" => "\u{e4}",
        "ouml" => "\u{f6}",
        "uuml" => "\u{fc}",
        "aacute" => "\u{e1}",
        "eacute" => "\u{e9}",
        "iacute" => "\u{ed}",
        "oacute" => "\u{f3}",
        "uacute" => "\u{fa}",
        "acirc" => "\u{e2}",
        "ecirc" => "\u{ea}",
        "icirc" => "\u{ee}",
        "ocirc" => "\u{f4}",
        "ucirc" => "\u{fb}",
        "atilde" => "\u{e3}",
        "otilde" => "\u{f5}",
        "ntilde" => "\u{f1}",
        "ccedil" => "\u{e7}",
        "szlig" => "\u{df}",
        "agrave" => "\u{e0}",
        "egrave" => "\u{e8}",
        "igrave" => "\u{ec}",
        "ograve" => "\u{f2}",
        "ugrave" => "\u{f9}",
        "AElig" => "\u{c6}",
        "Aacute" => "\u{c1}",
        "Eacute" => "\u{c9}",
        "Iacute" => "\u{cd}",
        "Oacute" => "\u{d3}",
        "Uacute" => "\u{da}",
        "Acirc" => "\u{c2}",
        "Ecirc" => "\u{ca}",
        "Icirc" => "\u{ce}",
        "Ocirc" => "\u{d4}",
        "Ucirc" => "\u{db}",
        "Atilde" => "\u{c3}",
        "Otilde" => "\u{d5}",
        "Ntilde" => "\u{d1}",
        "Ccedil" => "\u{c7}",
        "Aring" => "\u{c5}",
        "aring" => "\u{e5}",
        "Oslash" => "\u{d8}",
        "oslash" => "\u{f8}",
        "ETH" => "\u{d0}",
        "eth" => "\u{f0}",
        "THORN" => "\u{de}",
        "thorn" => "\u{fe}",
        "yuml" => "\u{ff}",
        "iexcl" => "\u{a1}",
        "iquest" => "\u{bf}",
        "uml" => "\u{a8}",
        "acute" => "\u{b4}",
        "cedil" => "\u{b8}",
        "macr" => "\u{af}",
        "tilde" => "\u{2dc}",
        "circ" => "\u{2c6}",
        "zwnj" => "\u{200c}",
        "zwj" => "\u{200d}",
        "lrm" => "\u{200e}",
        "rlm" => "\u{200f}",
        "alpha" => "\u{3b1}",
        "beta" => "\u{3b2}",
        "gamma" => "\u{3b3}",
        "delta" => "\u{3b4}",
        "epsilon" => "\u{3b5}",
        "zeta" => "\u{3b6}",
        "eta" => "\u{3b7}",
        "theta" => "\u{3b8}",
        "iota" => "\u{3b9}",
        "kappa" => "\u{3ba}",
        "lambda" => "\u{3bb}",
        "mu" => "\u{3bc}",
        "nu" => "\u{3bd}",
        "xi" => "\u{3be}",
        "omicron" => "\u{3bf}",
        "pi" => "\u{3c0}",
        "rho" => "\u{3c1}",
        "sigma" => "\u{3c3}",
        "tau" => "\u{3c4}",
        "upsilon" => "\u{3c5}",
        "phi" => "\u{3c6}",
        "chi" => "\u{3c7}",
        "psi" => "\u{3c8}",
        "omega" => "\u{3c9}",
        "Alpha" => "\u{391}",
        "Beta" => "\u{392}",
        "Gamma" => "\u{393}",
        "Delta" => "\u{394}",
        "Epsilon" => "\u{395}",
        "Zeta" => "\u{396}",
        "Eta" => "\u{397}",
        "Theta" => "\u{398}",
        "Iota" => "\u{399}",
        "Kappa" => "\u{39a}",
        "Lambda" => "\u{39b}",
        "Mu" => "\u{39c}",
        "Nu" => "\u{39d}",
        "Xi" => "\u{39e}",
        "Omicron" => "\u{39f}",
        "Pi" => "\u{3a0}",
        "Rho" => "\u{3a1}",
        "Sigma" => "\u{3a3}",
        "Tau" => "\u{3a4}",
        "Upsilon" => "\u{3a5}",
        "Phi" => "\u{3a6}",
        "Chi" => "\u{3a7}",
        "Psi" => "\u{3a8}",
        "Omega" => "\u{3a9}",
        "sigmaf" => "\u{3c2}",
        "thetasym" => "\u{3d1}",
        "upsih" => "\u{3d2}",
        "piv" => "\u{3d6}",
        _ => return None,
    };
    Some(s.to_string())
}

pub(crate) fn parser_unescape_entities(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let strict = bool_of(vm, args[1]);
    Ok(new_str(vm, &unescape_entities(&s, strict)))
}

pub(crate) fn entities_unescape(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    Ok(new_str(vm, &unescape_entities(&s, false)))
}

pub(crate) fn parser_factory(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lorg/jsoup/parser/Parser;", Native::Opaque)
}

pub(crate) fn safelist_none(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lorg/jsoup/safety/Safelist;", Native::Opaque)
}

pub(crate) fn validate_not_empty(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    if s.is_empty() {
        return Err(iae(vm, "String must not be empty"));
    }
    Ok(JValue::Null)
}

pub(crate) fn validate_not_null(vm: &mut Vm, args: &[JValue]) -> R {
    if args[0] == JValue::Null {
        return Err(iae(vm, "Object must not be null"));
    }
    Ok(JValue::Null)
}

pub(crate) fn query_parser_parse(vm: &mut Vm, args: &[JValue]) -> R {
    let sel = jstr(vm, args[0])?;
    alloc(vm, "Lorg/jsoup/select/Evaluator;", Native::Str(sel))
}

pub(crate) fn evaluator_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::Str(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &s))
}

pub(crate) fn evaluator_init(vm: &mut Vm, args: &[JValue]) -> R {
    let this = args[0].as_obj();
    let s = jstr(vm, args[1])?;
    vm.arena.objects[this as usize].native = Some(Native::Str(s));
    Ok(JValue::Null)
}

pub(crate) fn textnode_init(vm: &mut Vm, args: &[JValue]) -> R {
    let this = args[0].as_obj();
    let s = jstr(vm, args[1])?;
    vm.arena.objects[this as usize].native = Some(Native::Str(s));
    Ok(JValue::Null)
}

pub(crate) fn elements_init(vm: &mut Vm, args: &[JValue]) -> R {
    let this = args[0].as_obj();
    vm.arena.objects[this as usize].native = Some(Native::JsoupElements {
        doc: JsoupDocRef::new(Document::from("")),
        ids: Vec::new(),
    });
    Ok(JValue::Null)
}

pub(crate) fn textnode_text(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::Str(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &s))
}

pub(crate) fn attribute_get_key(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Pair(k, _)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(*k)
}

pub(crate) fn attribute_get_value(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Pair(_, v)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(*v)
}

pub(crate) fn attributes_as_list(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::List(items)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    list_alloc(vm, items.clone())
}

pub(crate) fn jsoup_node_name(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::JsoupElement { doc, id }) => {
            let n = node_ref_of(&doc.doc, *id);
            let name = if n.is_text() {
                "#text".to_string()
            } else {
                n.node_name().unwrap_or_default().to_lowercase()
            };
            Ok(new_str(vm, &name))
        }
        Some(Native::JsoupDoc(_)) => Ok(new_str(vm, "#document")),
        Some(Native::Str(_)) => Ok(new_str(vm, "#text")),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn element_whole_text(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let text = whole_text_of(node_ref_of(&doc.doc, id));
    Ok(new_str(vm, &text))
}

pub(crate) fn document_whole_text(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let text = whole_text_of(doc.doc.root());
    Ok(new_str(vm, &text))
}

pub(crate) fn element_whole_own_text(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    let text: String = node
        .children()
        .iter()
        .filter(|c| c.is_text())
        .map(|c| c.text().to_string())
        .collect();
    Ok(new_str(vm, &text))
}

pub(crate) fn element_has_class(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let cls = jstr(vm, args[1])?;
    let has = node_ref_of(&doc.doc, id).has_class(&cls);
    Ok(JValue::Int(i32::from(has)))
}

pub(crate) fn elements_has_class(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = elements_payload(vm, args[0])?;
    let cls = jstr(vm, args[1])?;
    let d = &*doc.doc;
    let has = ids.iter().any(|id| node_ref_of(d, *id).has_class(&cls));
    Ok(JValue::Int(i32::from(has)))
}

pub(crate) fn elements_remove(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = elements_payload(vm, args[0])?;
    let d = &*doc.doc;
    for id in &ids {
        node_ref_of(d, *id).remove_from_parent();
    }
    Ok(args[0])
}

pub(crate) fn element_child(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let idx = int_of(vm, args[1]);
    let kids = node_ref_of(&doc.doc, id).element_children();
    match kids.get(idx as usize) {
        Some(k) => alloc(
            vm,
            "Lorg/jsoup/nodes/Element;",
            Native::JsoupElement {
                doc: doc.clone(),
                id: k.id,
            },
        ),
        None => Err(aioobe(vm, idx, kids.len() as i32)),
    }
}

pub(crate) fn document_body(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let body = {
        let d = &*doc.doc;
        d.body().map(|n| n.id)
    };
    match body {
        Some(id) => alloc(
            vm,
            "Lorg/jsoup/nodes/Element;",
            Native::JsoupElement { doc, id },
        ),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn document_head(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let head = {
        let d = &*doc.doc;
        d.head().map(|n| n.id)
    };
    match head {
        Some(id) => alloc(
            vm,
            "Lorg/jsoup/nodes/Element;",
            Native::JsoupElement { doc, id },
        ),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn element_select_eval(vm: &mut Vm, args: &[JValue]) -> R {
    let selector = eval_selector_of(vm, args[1])?;
    element_select_impl(vm, args, selector)
}

pub(crate) fn document_select_eval(vm: &mut Vm, args: &[JValue]) -> R {
    let selector = eval_selector_of(vm, args[1])?;
    document_select_impl(vm, args, selector)
}

pub(crate) fn select_first_eval(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let node = payload(vm, args[0])
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let selector = eval_selector_of(vm, args[1])?;
    select_first_at(vm, doc, node, &selector)
}

pub(crate) fn collector_find_first(vm: &mut Vm, args: &[JValue]) -> R {
    let selector = eval_selector_of(vm, args[0])?;
    let doc = doc_of(vm, args[1])?;
    let node = payload(vm, args[1])
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    select_first_at(vm, doc, node, &selector)
}

pub(crate) fn element_next_sibling_node(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    match node.next_sibling() {
        Some(s) if s.is_element() => alloc(
            vm,
            "Lorg/jsoup/nodes/Element;",
            Native::JsoupElement {
                doc: doc.clone(),
                id: s.id,
            },
        ),
        Some(s) if s.is_text() => textnode_alloc(vm, s.text().as_ref()),
        Some(s) => alloc(
            vm,
            "Lorg/jsoup/nodes/Element;",
            Native::JsoupElement {
                doc: doc.clone(),
                id: s.id,
            },
        ),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn element_closest(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let sel = jstr(vm, args[1])?;
    let Some(matcher) = select_selector(&normalize_contains(&sel)) else {
        return Ok(JValue::Null);
    };
    let d = &*doc.doc;
    let mut cur = Some(node_ref_of(d, id));
    while let Some(n) = cur {
        if n.is_element() && matcher.match_element(&n) {
            return alloc(
                vm,
                "Lorg/jsoup/nodes/Element;",
                Native::JsoupElement {
                    doc: doc.clone(),
                    id: n.id,
                },
            );
        }
        cur = n.parent();
    }
    Ok(JValue::Null)
}

pub(crate) fn element_set_attr(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let name = jstr(vm, args[1])?;
    let value = jstr(vm, args[2])?;
    node_ref_of(&doc.doc, id).set_attr(&name, &value);
    Ok(args[0])
}

pub(crate) fn elements_set_attr(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = elements_payload(vm, args[0])?;
    let name = jstr(vm, args[1])?;
    let value = jstr(vm, args[2])?;
    let d = &*doc.doc;
    for id in &ids {
        node_ref_of(d, *id).set_attr(&name, &value);
    }
    Ok(args[0])
}

pub(crate) fn elements_html(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = elements_payload(vm, args[0])?;
    let d = &*doc.doc;
    match ids.first() {
        Some(id) => Ok(new_str(vm, node_ref_of(d, *id).html().as_ref())),
        None => Ok(new_str(vm, "")),
    }
}

pub(crate) fn element_text_nodes(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let d = &*doc.doc;
    let items = node_ref_of(d, id)
        .children()
        .iter()
        .filter(|c| c.is_text())
        .map(|c| textnode_alloc(vm, c.text().as_ref()))
        .collect::<Result<Vec<_>, _>>()?;
    list_alloc(vm, items)
}

pub(crate) fn element_child_nodes(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let d = &*doc.doc;
    let mut items = Vec::new();
    for c in node_ref_of(d, id).children() {
        if c.is_element() {
            items.push(alloc(
                vm,
                "Lorg/jsoup/nodes/Element;",
                Native::JsoupElement {
                    doc: doc.clone(),
                    id: c.id,
                },
            )?);
        } else if c.is_text() {
            items.push(textnode_alloc(vm, c.text().as_ref())?);
        }
    }
    list_alloc(vm, items)
}

pub(crate) fn element_owner_document(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    alloc(vm, "Lorg/jsoup/nodes/Document;", Native::JsoupDoc(doc))
}

pub(crate) fn document_create_element(vm: &mut Vm, args: &[JValue]) -> R {
    let tag = jstr(vm, args[1])?;
    let frag = Document::fragment(format!("<{tag}></{tag}>"));
    let id = fragment_first_element(&frag).ok_or_else(|| iae(vm, format!("bad tag {tag}")))?;
    alloc(
        vm,
        "Lorg/jsoup/nodes/Element;",
        Native::JsoupElement {
            doc: JsoupDocRef::new(frag),
            id,
        },
    )
}

pub(crate) fn elements_clone(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = elements_payload(vm, args[0])?;
    if ids.len() > 1 {
        log::warn!(
            "jsoup Elements.clone: {} elements, only the first is cloned",
            ids.len()
        );
    }
    match ids.first() {
        Some(id) => {
            let frag = node_ref_of(&doc.doc, *id).to_fragment();
            let refd = JsoupDocRef::new(frag);
            let Some(cid) = fragment_first_element(&refd.doc) else {
                return jsoup_elements(vm, refd, Vec::new());
            };
            jsoup_elements(vm, refd, vec![cid])
        }
        None => jsoup_elements(vm, doc, Vec::new()),
    }
}

pub(crate) fn element_attributes(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let d = &*doc.doc;
    let mut items = Vec::new();
    for a in node_ref_of(d, id).attrs() {
        items.push(attribute_alloc(
            vm,
            a.name.local.as_ref(),
            a.value.as_ref(),
        )?);
    }
    alloc(vm, "Lorg/jsoup/nodes/Attributes;", Native::List(items))
}

pub(crate) fn element_remove(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    node_ref_of(&doc.doc, id).remove_from_parent();
    Ok(JValue::Null)
}

pub(crate) fn element_class_names(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let classes = node_ref_of(&doc.doc, id)
        .class()
        .map(|c| {
            c.split_whitespace()
                .map(|s| new_str(vm, s))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    set_alloc(vm, classes)
}

pub(crate) fn elements_has_text(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = elements_payload(vm, args[0])?;
    let d = &*doc.doc;
    let has = ids
        .iter()
        .any(|id| !soup_text(node_ref_of(d, *id)).trim().is_empty());
    Ok(JValue::Int(i32::from(has)))
}

pub(crate) fn elements_prepend(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = elements_payload(vm, args[0])?;
    let html = jstr(vm, args[1])?;
    let d = &*doc.doc;
    for id in &ids {
        node_ref_of(d, *id).prepend_html(html.as_str());
    }
    Ok(args[0])
}

pub(crate) fn jsoup_parse_parser(vm: &mut Vm, args: &[JValue]) -> R {
    jsoup_parse_string(vm, &[args[0], args[1]])
}

pub(crate) fn jsoup_clean(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let doc = Document::from(s.as_str());
    let root = doc.root();
    // Match the security-critical part of Jsoup's Cleaner: unsafe executable
    // and embedded-content nodes are discarded, while ordinary markup stays.
    for selector in [
        "script", "style", "iframe", "object", "embed", "applet", "form",
    ] {
        if let Some(matcher) = select_selector(selector) {
            let ids: Vec<_> = Selection::from(root)
                .select_matcher(&matcher)
                .nodes()
                .iter()
                .map(|n| n.id)
                .collect();
            for id in ids {
                NodeRef::new(id, &doc.tree).remove_from_parent();
            }
        }
    }
    Ok(new_str(
        vm,
        doc.body()
            .map(|b| b.inner_html().to_string())
            .unwrap_or_default()
            .as_str(),
    ))
}

pub(crate) fn document_base_uri(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    Ok(new_str(vm, doc.base.as_deref().unwrap_or("")))
}

pub(crate) fn document_get_element_by_id(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let want = jstr(vm, args[1])?;
    let d = &*doc.doc;
    let root = d.root();
    let candidates = std::iter::once(root).chain(root.descendants());
    for node in candidates {
        if node.id_attr().as_deref() == Some(want.as_str()) {
            return alloc(
                vm,
                "Lorg/jsoup/nodes/Element;",
                Native::JsoupElement {
                    doc: doc.clone(),
                    id: node.id,
                },
            );
        }
    }
    Ok(JValue::Null)
}

pub(crate) fn elements_has_attr(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = elements_payload(vm, args[0])?;
    let name = jstr(vm, args[1])?;
    let name = name.strip_prefix("abs:").unwrap_or(&name);
    let d = &*doc.doc;
    let has = ids.iter().any(|id| node_ref_of(d, *id).has_attr(name));
    Ok(JValue::Int(i32::from(has)))
}

pub(crate) fn document_get_elements_by_class(vm: &mut Vm, args: &[JValue]) -> R {
    elements_by_class(vm, args, false)
}

pub(crate) fn element_get_elements_by_class(vm: &mut Vm, args: &[JValue]) -> R {
    elements_by_class(vm, args, true)
}

fn elements_by_class(vm: &mut Vm, args: &[JValue], include_self: bool) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let cls = jstr(vm, args[1])?;
    elements_matching(vm, doc, id, include_self, |n| n.has_class(&cls))
}

pub(crate) fn document_get_elements_by_tag(vm: &mut Vm, args: &[JValue]) -> R {
    elements_by_tag(vm, args, false)
}

fn elements_by_tag(vm: &mut Vm, args: &[JValue], include_self: bool) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let tag = jstr(vm, args[1])?;
    elements_matching(vm, doc, id, include_self, |n| {
        n.node_name().is_some_and(|t| t.eq_ignore_ascii_case(&tag))
    })
}

pub(crate) fn document_set_base_uri(vm: &mut Vm, args: &[JValue]) -> R {
    let base = jstr(vm, args[1])?;
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::JsoupDoc(doc) => {
            doc.base = Some(base);
            Ok(JValue::Null)
        }
        _ => Err(npe(vm)),
    }
}

pub(crate) fn element_append(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let html = jstr(vm, args[1])?;
    node_ref_of(&doc.doc, id).append_html(html.as_str());
    Ok(args[0])
}

pub(crate) fn element_prepend(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let html = jstr(vm, args[1])?;
    node_ref_of(&doc.doc, id).prepend_html(html.as_str());
    Ok(args[0])
}

pub(crate) fn element_dataset(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let d = &*doc.doc;
    let mut entries = Vec::new();
    for a in node_ref_of(d, id).attrs() {
        let key = a.name.local.as_ref();
        if let Some(data_key) = key.strip_prefix("data-") {
            entries.push((new_str(vm, data_key), new_str(vm, a.value.as_ref())));
        }
    }
    alloc(vm, "Ljava/util/HashMap;", Native::Map(entries))
}

pub(crate) fn element_replace_with(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let other_html = match payload(vm, args[1]) {
        Some(Native::JsoupElement { doc: odoc, id: oid }) => {
            node_ref_of(&odoc.doc, *oid).html().to_string()
        }
        Some(Native::JsoupDoc(odoc)) => odoc.doc.root().html().to_string(),
        Some(Native::Str(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    node_ref_of(&doc.doc, id).replace_with_html(other_html.as_str());
    Ok(JValue::Null)
}

pub(crate) fn elements_not(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = elements_payload(vm, args[0])?;
    let sel = jstr(vm, args[1])?;
    let Some(matcher) = select_selector(&normalize_contains(&sel)) else {
        return Err(iae(vm, format!("invalid selector {sel}")));
    };
    let d = &*doc.doc;
    let out = ids
        .iter()
        .filter(|id| {
            let n = node_ref_of(d, **id);
            !(n.is_element() && matcher.match_element(&n))
        })
        .copied()
        .collect();
    jsoup_elements(vm, doc, out)
}

pub(crate) fn elements_next(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = elements_payload(vm, args[0])?;
    let d = &*doc.doc;
    let mut out = Vec::new();
    for id in &ids {
        if let Some(n) = node_ref_of(d, *id).next_element_sibling() {
            if !out.contains(&n.id) {
                out.push(n.id);
            }
        }
    }
    jsoup_elements(vm, doc, out)
}

pub(crate) fn element_get_elements_containing_text(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let needle = jstr(vm, args[1])?.to_lowercase();
    elements_matching(vm, doc, id, false, |n| {
        soup_text(*n).to_lowercase().contains(&needle)
    })
}

pub(crate) fn element_next_element_siblings(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let d = &*doc.doc;
    let mut ids = Vec::new();
    let mut cur = node_ref_of(d, id).next_element_sibling();
    while let Some(n) = cur {
        ids.push(n.id);
        cur = n.next_element_sibling();
    }
    jsoup_elements(vm, doc, ids)
}

pub(crate) fn element_previous_element_siblings(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let d = &*doc.doc;
    let mut ids = Vec::new();
    let mut cur = node_ref_of(d, id).prev_element_sibling();
    while let Some(n) = cur {
        ids.push(n.id);
        cur = n.prev_element_sibling();
    }
    jsoup_elements(vm, doc, ids)
}

pub(crate) fn elements_select_first(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = elements_payload(vm, args[0])?;
    let sel = jsoup_first_selector_arg(vm, args)?;
    for id in &ids {
        let hit = select_first_at(vm, doc.clone(), *id, &sel)?;
        if hit != JValue::Null {
            return Ok(hit);
        }
    }
    Ok(JValue::Null)
}

pub(crate) fn document_clone(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let html = doc.doc.html().to_string();
    let mut nd = JsoupDocRef::new(Document::from(html));
    nd.base = doc.base.clone();
    alloc(vm, "Lorg/jsoup/nodes/Document;", Native::JsoupDoc(nd))
}

pub(crate) fn document_select_xpath(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    log::warn!("jsoup selectXpath is not supported; returning empty Elements");
    jsoup_elements(vm, doc, Vec::new())
}

pub(crate) fn element_base_uri(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    Ok(new_str(vm, doc.base.as_deref().unwrap_or("")))
}

pub(crate) fn element_clone(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let frag = node_ref_of(&doc.doc, id).to_fragment();
    let refd = JsoupDocRef::new(frag);
    let cid = fragment_first_element(&refd.doc).ok_or_else(|| iae(vm, "clone failed"))?;
    alloc(
        vm,
        "Lorg/jsoup/nodes/Element;",
        Native::JsoupElement { doc: refd, id: cid },
    )
}

pub(crate) fn element_get_all_elements(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    elements_matching(vm, doc, id, true, |_| true)
}

pub(crate) fn element_set_html(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let html = jstr(vm, args[1])?;
    node_ref_of(&doc.doc, id).set_html(html.as_str());
    Ok(args[0])
}

pub(crate) fn element_is(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let sel = jstr(vm, args[1])?;
    let node = node_ref_of(&doc.doc, id);
    let matched = node.is_element() && node.is(&normalize_contains(&sel));
    Ok(JValue::Int(i32::from(matched)))
}

pub(crate) fn element_normal_name(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let name = node_ref_of(&doc.doc, id)
        .node_name()
        .unwrap_or_default()
        .to_lowercase();
    Ok(new_str(vm, &name))
}

pub(crate) fn element_prepend_element(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let tag = jstr(vm, args[1])?;
    let frag = Document::fragment(format!("<{tag}></{tag}>"));
    let fid = fragment_first_element(&frag).ok_or_else(|| iae(vm, format!("bad tag {tag}")))?;
    let html = node_ref_of(&frag, fid).html().to_string();
    node_ref_of(&doc.doc, id).prepend_html(html.as_str());
    alloc(
        vm,
        "Lorg/jsoup/nodes/Element;",
        Native::JsoupElement {
            doc: JsoupDocRef::new(frag),
            id: fid,
        },
    )
}

pub(crate) fn element_tag(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, id) = element_payload(vm, args[0])?;
    let name = node_ref_of(&doc.doc, id)
        .node_name()
        .unwrap_or_default()
        .to_lowercase();
    alloc(vm, "Lorg/jsoup/parser/Tag;", Native::Str(name))
}

pub(crate) fn tag_normal_name(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::Str(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &s))
}

pub(crate) fn elements_remove_first(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = elements_payload(vm, args[0])?;
    match ids.first() {
        Some(id) => {
            node_ref_of(&doc.doc, *id).remove_from_parent();
            alloc(
                vm,
                "Lorg/jsoup/nodes/Element;",
                Native::JsoupElement { doc, id: *id },
            )
        }
        None => Ok(JValue::Null),
    }
}

// ---------------------------------------------------------------------------
// org.jsoup native table
// ---------------------------------------------------------------------------

pub(crate) const JSOUP_TABLE: &[NativeEntry] = &[
    ne!(
        "Lorg/jsoup/Jsoup;",
        "parse",
        "(Ljava/lang/String;Ljava/lang/String;)Lorg/jsoup/nodes/Document;",
        false,
        jsoup_parse_string
    ),
    ne!(
        "Lorg/jsoup/Jsoup;",
        "parse",
        "(Ljava/lang/String;)Lorg/jsoup/nodes/Document;",
        false,
        jsoup_parse_string_default
    ),
    ne!(
        "Lorg/jsoup/Jsoup;",
        "parseBodyFragment",
        "(Ljava/lang/String;)Lorg/jsoup/nodes/Document;",
        false,
        jsoup_parse_body_fragment
    ),
    ne!(
        "Lorg/jsoup/Jsoup;",
        "parseBodyFragment",
        "(Ljava/lang/String;Ljava/lang/String;)Lorg/jsoup/nodes/Document;",
        false,
        jsoup_parse_body_fragment
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "select",
        "(Ljava/lang/String;)Lorg/jsoup/select/Elements;",
        true,
        document_select
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "text",
        "()Ljava/lang/String;",
        true,
        document_text
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "location",
        "()Ljava/lang/String;",
        true,
        document_location
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "title",
        "()Ljava/lang/String;",
        true,
        document_title
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "select",
        "(Ljava/lang/String;)Lorg/jsoup/select/Elements;",
        true,
        element_select
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "selectFirst",
        "(Ljava/lang/String;)Lorg/jsoup/nodes/Element;",
        true,
        element_select_first
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "text",
        "()Ljava/lang/String;",
        true,
        element_text
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "data",
        "()Ljava/lang/String;",
        true,
        element_data
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "val",
        "()Ljava/lang/String;",
        true,
        element_val
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "attr",
        "(Ljava/lang/String;)Ljava/lang/String;",
        true,
        element_attr
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "absUrl",
        "(Ljava/lang/String;)Ljava/lang/String;",
        true,
        element_abs_url
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "parent",
        "()Lorg/jsoup/nodes/Element;",
        true,
        element_parent
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "parents",
        "()Lorg/jsoup/select/Elements;",
        true,
        element_parents
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "previousElementSibling",
        "()Lorg/jsoup/nodes/Element;",
        true,
        element_previous_sibling
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "nextElementSibling",
        "()Lorg/jsoup/nodes/Element;",
        true,
        element_next_sibling
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "children",
        "()Lorg/jsoup/select/Elements;",
        true,
        element_children
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "hasAttr",
        "(Ljava/lang/String;)Z",
        true,
        element_has_attr
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "id",
        "()Ljava/lang/String;",
        true,
        element_id_attr
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "tagName",
        "()Ljava/lang/String;",
        true,
        element_tag_name
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "ownText",
        "()Ljava/lang/String;",
        true,
        element_own_text
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "html",
        "()Ljava/lang/String;",
        true,
        element_html
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "outerHtml",
        "()Ljava/lang/String;",
        true,
        element_outer_html
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "first",
        "()Lorg/jsoup/nodes/Element;",
        true,
        elements_first
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "get",
        "(I)Lorg/jsoup/nodes/Element;",
        true,
        elements_get
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "get",
        "(I)Ljava/lang/Object;",
        true,
        elements_get
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "size",
        "()I",
        true,
        elements_size
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "isEmpty",
        "()Z",
        true,
        elements_is_empty
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "text",
        "()Ljava/lang/String;",
        true,
        elements_text
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "last",
        "()Lorg/jsoup/nodes/Element;",
        true,
        elements_last
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "eachText",
        "()Ljava/util/List;",
        true,
        elements_each_text
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "attr",
        "(Ljava/lang/String;)Ljava/lang/String;",
        true,
        elements_attr
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "eachAttr",
        "(Ljava/lang/String;)Ljava/util/List;",
        true,
        elements_each_attr
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "select",
        "(Ljava/lang/String;)Lorg/jsoup/select/Elements;",
        true,
        elements_select
    ),
    ne!(
        "Lorg/jsoup/parser/Parser;",
        "unescapeEntities",
        "(Ljava/lang/String;Z)Ljava/lang/String;",
        false,
        parser_unescape_entities
    ),
    ne!(
        "Lorg/jsoup/parser/Parser;",
        "xmlParser",
        "()Lorg/jsoup/parser/Parser;",
        false,
        parser_factory
    ),
    ne!(
        "Lorg/jsoup/parser/Parser;",
        "htmlParser",
        "()Lorg/jsoup/parser/Parser;",
        false,
        parser_factory
    ),
    ne!(
        "Lorg/jsoup/nodes/Entities;",
        "unescape",
        "(Ljava/lang/String;)Ljava/lang/String;",
        false,
        entities_unescape
    ),
    ne!(
        "Lorg/jsoup/helper/Validate;",
        "notEmpty",
        "(Ljava/lang/String;)V",
        false,
        validate_not_empty
    ),
    ne!(
        "Lorg/jsoup/helper/Validate;",
        "notNull",
        "(Ljava/lang/Object;)V",
        false,
        validate_not_null
    ),
    ne!(
        "Lorg/jsoup/safety/Safelist;",
        "none",
        "()Lorg/jsoup/safety/Safelist;",
        false,
        safelist_none
    ),
    ne!(
        "Lorg/jsoup/select/QueryParser;",
        "parse",
        "(Ljava/lang/String;)Lorg/jsoup/select/Evaluator;",
        false,
        query_parser_parse
    ),
    ne!(
        "Lorg/jsoup/select/Collector;",
        "findFirst",
        "(Lorg/jsoup/select/Evaluator;Lorg/jsoup/nodes/Element;)Lorg/jsoup/nodes/Element;",
        false,
        collector_find_first
    ),
    ne!(
        "Lorg/jsoup/select/Evaluator;",
        "toString",
        "()Ljava/lang/String;",
        true,
        evaluator_to_string
    ),
    ne!(
        "Lorg/jsoup/select/Evaluator$Tag;",
        "<init>",
        "(Ljava/lang/String;)V",
        true,
        evaluator_init
    ),
    ne!(
        "Lorg/jsoup/select/Evaluator$Class;",
        "<init>",
        "(Ljava/lang/String;)V",
        true,
        evaluator_init
    ),
    ne!(
        "Lorg/jsoup/select/Evaluator$Id;",
        "<init>",
        "(Ljava/lang/String;)V",
        true,
        evaluator_init
    ),
    ne!(
        "Lorg/jsoup/nodes/TextNode;",
        "<init>",
        "(Ljava/lang/String;)V",
        true,
        textnode_init
    ),
    ne!(
        "Lorg/jsoup/nodes/TextNode;",
        "text",
        "()Ljava/lang/String;",
        true,
        textnode_text
    ),
    ne!(
        "Lorg/jsoup/nodes/TextNode;",
        "getWholeText",
        "()Ljava/lang/String;",
        true,
        textnode_text
    ),
    ne!(
        "Lorg/jsoup/nodes/Attribute;",
        "getKey",
        "()Ljava/lang/String;",
        true,
        attribute_get_key
    ),
    ne!(
        "Lorg/jsoup/nodes/Attribute;",
        "getValue",
        "()Ljava/lang/String;",
        true,
        attribute_get_value
    ),
    ne!(
        "Lorg/jsoup/nodes/Attributes;",
        "asList",
        "()Ljava/util/List;",
        true,
        attributes_as_list
    ),
    ne!(
        "Lorg/jsoup/nodes/Node;",
        "nodeName",
        "()Ljava/lang/String;",
        true,
        jsoup_node_name
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "nodeName",
        "()Ljava/lang/String;",
        true,
        jsoup_node_name
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "nodeName",
        "()Ljava/lang/String;",
        true,
        jsoup_node_name
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "wholeText",
        "()Ljava/lang/String;",
        true,
        element_whole_text
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "wholeText",
        "()Ljava/lang/String;",
        true,
        document_whole_text
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "wholeOwnText",
        "()Ljava/lang/String;",
        true,
        element_whole_own_text
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "hasClass",
        "(Ljava/lang/String;)Z",
        true,
        element_has_class
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "hasClass",
        "(Ljava/lang/String;)Z",
        true,
        elements_has_class
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "remove",
        "()Lorg/jsoup/select/Elements;",
        true,
        elements_remove
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "<init>",
        "()V",
        true,
        elements_init
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "child",
        "(I)Lorg/jsoup/nodes/Element;",
        true,
        element_child
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "body",
        "()Lorg/jsoup/nodes/Element;",
        true,
        document_body
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "head",
        "()Lorg/jsoup/nodes/Element;",
        true,
        document_head
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "select",
        "(Lorg/jsoup/select/Evaluator;)Lorg/jsoup/select/Elements;",
        true,
        element_select_eval
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "select",
        "(Lorg/jsoup/select/Evaluator;)Lorg/jsoup/select/Elements;",
        true,
        document_select_eval
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "selectFirst",
        "(Lorg/jsoup/select/Evaluator;)Lorg/jsoup/nodes/Element;",
        true,
        select_first_eval
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "selectFirst",
        "(Lorg/jsoup/select/Evaluator;)Lorg/jsoup/nodes/Element;",
        true,
        select_first_eval
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "nextSibling",
        "()Lorg/jsoup/nodes/Node;",
        true,
        element_next_sibling_node
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "closest",
        "(Ljava/lang/String;)Lorg/jsoup/nodes/Element;",
        true,
        element_closest
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "attr",
        "(Ljava/lang/String;Ljava/lang/String;)Lorg/jsoup/nodes/Element;",
        true,
        element_set_attr
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "attr",
        "(Ljava/lang/String;Ljava/lang/String;)Lorg/jsoup/select/Elements;",
        true,
        elements_set_attr
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "html",
        "()Ljava/lang/String;",
        true,
        elements_html
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "textNodes",
        "()Ljava/util/List;",
        true,
        element_text_nodes
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "childNodes",
        "()Ljava/util/List;",
        true,
        element_child_nodes
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "ownerDocument",
        "()Lorg/jsoup/nodes/Document;",
        true,
        element_owner_document
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "createElement",
        "(Ljava/lang/String;)Lorg/jsoup/nodes/Element;",
        true,
        document_create_element
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "clone",
        "()Lorg/jsoup/select/Elements;",
        true,
        elements_clone
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "attributes",
        "()Lorg/jsoup/nodes/Attributes;",
        true,
        element_attributes
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "remove",
        "()V",
        true,
        element_remove
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "classNames",
        "()Ljava/util/Set;",
        true,
        element_class_names
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "hasText",
        "()Z",
        true,
        elements_has_text
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "prepend",
        "(Ljava/lang/String;)Lorg/jsoup/select/Elements;",
        true,
        elements_prepend
    ),
    ne!(
        "Lorg/jsoup/Jsoup;",
        "parse",
        "(Ljava/lang/String;Ljava/lang/String;Lorg/jsoup/parser/Parser;)Lorg/jsoup/nodes/Document;",
        false,
        jsoup_parse_parser
    ),
    ne!(
        "Lorg/jsoup/Jsoup;",
        "clean",
        "(Ljava/lang/String;Lorg/jsoup/safety/Safelist;)Ljava/lang/String;",
        false,
        jsoup_clean
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "baseUri",
        "()Ljava/lang/String;",
        true,
        document_base_uri
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "baseUri",
        "()Ljava/lang/String;",
        true,
        element_base_uri
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "getElementById",
        "(Ljava/lang/String;)Lorg/jsoup/nodes/Element;",
        true,
        document_get_element_by_id
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "hasAttr",
        "(Ljava/lang/String;)Z",
        true,
        elements_has_attr
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "getElementsByClass",
        "(Ljava/lang/String;)Lorg/jsoup/select/Elements;",
        true,
        document_get_elements_by_class
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "getElementsByClass",
        "(Ljava/lang/String;)Lorg/jsoup/select/Elements;",
        true,
        element_get_elements_by_class
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "getElementsByTag",
        "(Ljava/lang/String;)Lorg/jsoup/select/Elements;",
        true,
        document_get_elements_by_tag
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "setBaseUri",
        "(Ljava/lang/String;)V",
        true,
        document_set_base_uri
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "append",
        "(Ljava/lang/String;)Lorg/jsoup/nodes/Element;",
        true,
        element_append
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "prepend",
        "(Ljava/lang/String;)Lorg/jsoup/nodes/Element;",
        true,
        element_prepend
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "dataset",
        "()Ljava/util/Map;",
        true,
        element_dataset
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "replaceWith",
        "(Lorg/jsoup/nodes/Node;)V",
        true,
        element_replace_with
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "not",
        "(Ljava/lang/String;)Lorg/jsoup/select/Elements;",
        true,
        elements_not
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "next",
        "()Lorg/jsoup/select/Elements;",
        true,
        elements_next
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "getElementsContainingText",
        "(Ljava/lang/String;)Lorg/jsoup/select/Elements;",
        true,
        element_get_elements_containing_text
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "nextElementSiblings",
        "()Lorg/jsoup/select/Elements;",
        true,
        element_next_element_siblings
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "previousElementSiblings",
        "()Lorg/jsoup/select/Elements;",
        true,
        element_previous_element_siblings
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "selectFirst",
        "(Ljava/lang/String;)Lorg/jsoup/nodes/Element;",
        true,
        elements_select_first
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "clone",
        "()Lorg/jsoup/nodes/Document;",
        true,
        document_clone
    ),
    ne!(
        "Lorg/jsoup/nodes/Document;",
        "selectXpath",
        "(Ljava/lang/String;)Lorg/jsoup/select/Elements;",
        true,
        document_select_xpath
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "clone",
        "()Lorg/jsoup/nodes/Element;",
        true,
        element_clone
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "getAllElements",
        "()Lorg/jsoup/select/Elements;",
        true,
        element_get_all_elements
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "html",
        "(Ljava/lang/String;)Lorg/jsoup/nodes/Element;",
        true,
        element_set_html
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "is",
        "(Ljava/lang/String;)Z",
        true,
        element_is
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "normalName",
        "()Ljava/lang/String;",
        true,
        element_normal_name
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "prependElement",
        "(Ljava/lang/String;)Lorg/jsoup/nodes/Element;",
        true,
        element_prepend_element
    ),
    ne!(
        "Lorg/jsoup/nodes/Element;",
        "tag",
        "()Lorg/jsoup/parser/Tag;",
        true,
        element_tag
    ),
    ne!(
        "Lorg/jsoup/parser/Tag;",
        "normalName",
        "()Ljava/lang/String;",
        true,
        tag_normal_name
    ),
    ne!(
        "Lorg/jsoup/select/Elements;",
        "removeFirst",
        "()Ljava/lang/Object;",
        true,
        elements_remove_first
    ),
];

#[cfg(test)]
mod tests;
