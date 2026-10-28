use std::cell::RefCell;
use std::rc::{Rc, Weak};

use html5ever::{LocalName, QualName, ns};
use markup5ever_rcdom::{Handle, Node, NodeData};

pub(crate) fn tag(node: &Node) -> Option<&str> {
    match &node.data {
        NodeData::Element { name, .. } => Some(&name.local),
        _ => None,
    }
}

pub(crate) fn is(node: &Node, names: &[&str]) -> bool {
    tag(node).is_some_and(|tag| names.contains(&tag))
}

pub(crate) fn attribute(node: &Node, name: &str) -> Option<String> {
    match &node.data {
        NodeData::Element { attrs, .. } => attrs
            .borrow()
            .iter()
            .find(|attr| &*attr.name.local == name)
            .map(|attr| attr.value.to_string()),
        _ => None,
    }
}

pub(crate) fn style(node: &Node) -> String {
    attribute(node, "style").unwrap_or_default()
}

pub(crate) fn parent(node: &Node) -> Option<Handle> {
    let weak = node.parent.take();
    let parent = weak.as_ref().and_then(Weak::upgrade);
    node.parent.set(weak);
    parent
}

pub(crate) fn element_children(node: &Node) -> Vec<Handle> {
    node.children
        .borrow()
        .iter()
        .filter(|child| tag(child).is_some())
        .cloned()
        .collect()
}

pub(crate) fn elements(node: &Handle) -> Vec<Handle> {
    let mut out = Vec::new();
    for child in node.children.borrow().iter() {
        if tag(child).is_some() {
            out.push(Rc::clone(child));
        }
        out.extend(elements(child));
    }
    out
}

pub(crate) fn text_content(node: &Node) -> String {
    let mut out = String::new();
    collect_text(node, &mut out);
    out
}

fn collect_text(node: &Node, out: &mut String) {
    if let NodeData::Text { contents } = &node.data {
        out.push_str(&contents.borrow());
    }
    for child in node.children.borrow().iter() {
        collect_text(child, out);
    }
}

pub(crate) fn closest(node: &Handle, names: &[&str]) -> bool {
    let mut at = Some(Rc::clone(node));
    while let Some(current) = at {
        if is(&current, names) {
            return true;
        }
        at = parent(&current);
    }
    false
}

pub(crate) fn has_following_content(node: &Handle) -> bool {
    let Some(parent) = parent(node) else {
        return false;
    };
    let siblings = parent.children.borrow();
    let after = siblings
        .iter()
        .skip_while(|sibling| !Rc::ptr_eq(sibling, node))
        .skip(1);
    for sibling in after {
        match &sibling.data {
            NodeData::Element { .. } => return true,
            NodeData::Text { contents } if !contents.borrow().trim().is_empty() => return true,
            _ => {}
        }
    }
    false
}

pub(crate) fn previous_element(node: &Handle) -> Option<Handle> {
    let parent = parent(node)?;
    let siblings = element_children(&parent);
    let at = siblings
        .iter()
        .position(|sibling| Rc::ptr_eq(sibling, node))?;
    at.checked_sub(1)
        .and_then(|index| siblings.get(index))
        .cloned()
}

pub(crate) fn create(name: &str) -> Handle {
    Node::new(NodeData::Element {
        name: QualName::new(None, ns!(html), LocalName::from(name)),
        attrs: RefCell::new(Vec::new()),
        template_contents: RefCell::new(None),
        mathml_annotation_xml_integration_point: false,
    })
}

pub(crate) fn remove(node: &Handle) {
    if let Some(parent) = parent(node) {
        parent
            .children
            .borrow_mut()
            .retain(|child| !Rc::ptr_eq(child, node));
    }
    node.parent.set(None);
}

pub(crate) fn append(parent: &Handle, child: &Handle) {
    remove(child);
    child.parent.set(Some(Rc::downgrade(parent)));
    parent.children.borrow_mut().push(Rc::clone(child));
}

pub(crate) fn insert_before(reference: &Handle, node: &Handle) {
    let Some(parent) = parent(reference) else {
        return;
    };
    remove(node);
    node.parent.set(Some(Rc::downgrade(&parent)));
    let mut children = parent.children.borrow_mut();
    let at = children
        .iter()
        .position(|child| Rc::ptr_eq(child, reference))
        .unwrap_or(children.len());
    children.insert(at, Rc::clone(node));
}

pub(crate) fn move_children(from: &Handle, to: &Handle) {
    let children: Vec<Handle> = from.children.borrow().clone();
    for child in &children {
        append(to, child);
    }
}

pub(crate) fn unwrap(node: &Handle) {
    let children: Vec<Handle> = node.children.borrow().clone();
    for child in &children {
        insert_before(node, child);
    }
    remove(node);
}

pub(crate) fn find(node: &Handle, name: &str) -> Option<Handle> {
    elements(node)
        .into_iter()
        .find(|element| is(element, &[name]))
}
