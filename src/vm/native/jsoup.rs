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
    Matcher::new(&normalize_contains(selector)).ok()
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
    let (doc, selector) = (doc_of(vm, args[0])?, jsoup_first_selector_arg(vm, args)?);
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
    let (doc, selector) = (doc_of(vm, args[0])?, jsoup_first_selector_arg(vm, args)?);
    let node_payload = payload(vm, args[0]);
    let Some(node_id) = node_payload.and_then(element_id_of) else {
        return Err(npe(vm));
    };
    let ids = {
        let d = &*doc.doc;
        let node = NodeRef::new(node_id, &d.tree);
        select_matches(d, node, &selector)
    };
    jsoup_elements(vm, doc, ids)
}

pub(crate) fn element_select_first(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let node_payload = payload(vm, args[0]);
    let Some(node) = node_payload.and_then(element_id_of) else {
        return Err(npe(vm));
    };
    let selector = jsoup_first_selector_arg(vm, args)?;
    let out_id = {
        let d = &*doc.doc;
        let node_ref = NodeRef::new(node, &d.tree);
        let Some(matcher) = select_selector(&selector) else {
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
// org.jsoup native table
// ---------------------------------------------------------------------------

pub(crate) const JSOUP_TABLE: &[NativeEntry] = &[
    ne!(
        "Leu/kanade/tachiyomi/util/JsoupExtensionsKt;",
        "asJsoup$default",
        "(Lokhttp3/Response;Ljava/lang/String;ILjava/lang/Object;)Lorg/jsoup/nodes/Document;",
        false,
        jsoup_parse
    ),
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
];

#[cfg(test)]
mod tests;
