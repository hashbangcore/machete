//! Variable lookup and scoping.
//!
//! Machete knows two levels: globals declared once for the whole site, and
//! locals supplied by a single component invocation. Looking up a name checks
//! the locals first, so a component can override a global without changing it
//! for anything else.

use crate::error::ScopeError;
use std::collections::BTreeMap;

/// Variable names paired with their values.
///
/// Machete does not infer types. Every value is a string, so `true` declared in
/// a component is the text `"true"` and nothing more.
pub type Bindings = BTreeMap<String, String>;

/// A set of variable values, split into globals and locals.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    globals: Bindings,
    locals: Bindings,
}

impl Scope {
    /// Builds a scope from site-wide variables, with no locals.
    pub fn new(globals: Bindings) -> Self {
        Self {
            globals,
            locals: Bindings::new(),
        }
    }

    /// Builds a scope with no variables at all.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Returns a scope that keeps the same globals and adds `locals`.
    pub fn child(&self, locals: Bindings) -> Self {
        Self {
            globals: self.globals.clone(),
            locals,
        }
    }

    /// Combines several sources of bindings into one set.
    ///
    /// Fails if any name appears in more than one source, since a component
    /// should not be able to contradict itself.
    pub fn merge(sources: &[Bindings]) -> Result<Bindings, ScopeError> {
        let mut merged = Bindings::new();

        for source in sources {
            for (name, value) in source {
                if merged.insert(name.clone(), value.clone()).is_some() {
                    return Err(ScopeError::DuplicateVariable { name: name.clone() });
                }
            }
        }

        Ok(merged)
    }

    /// Looks up a name, preferring the local value over the global one.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.locals
            .get(name)
            .or_else(|| self.globals.get(name))
            .map(String::as_str)
    }

    /// Returns the site-wide variables.
    pub fn globals(&self) -> &Bindings {
        &self.globals
    }

    /// Returns the variables local to this scope.
    pub fn locals(&self) -> &Bindings {
        &self.locals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> Bindings {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn a_local_wins_over_a_global() {
        let scope = Scope::new(vars(&[("title", "site")])).child(vars(&[("title", "page")]));
        assert_eq!(scope.get("title"), Some("page"));
    }

    #[test]
    fn a_global_is_still_reachable() {
        let scope = Scope::new(vars(&[("lang", "es")])).child(vars(&[("title", "page")]));
        assert_eq!(scope.get("lang"), Some("es"));
    }

    #[test]
    fn a_local_does_not_change_the_global() {
        let globals = vars(&[("title", "site")]);
        let scope = Scope::new(globals.clone()).child(vars(&[("title", "page")]));
        assert_eq!(globals.get("title").map(String::as_str), Some("site"));
        assert_eq!(
            scope.globals().get("title").map(String::as_str),
            Some("site")
        );
    }

    #[test]
    fn an_unknown_name_is_absent() {
        assert_eq!(Scope::new(Bindings::new()).get("nope"), None);
    }

    #[test]
    fn merging_rejects_a_name_from_two_sources() {
        let error = Scope::merge(&[vars(&[("a", "1")]), vars(&[("a", "2")])]);
        assert!(matches!(error, Err(ScopeError::DuplicateVariable { .. })));
    }

    #[test]
    fn merging_disjoint_sources_is_fine() {
        let merged = Scope::merge(&[vars(&[("a", "1")]), vars(&[("b", "2")])]).expect("no clash");
        assert_eq!(merged.get("a").map(String::as_str), Some("1"));
        assert_eq!(merged.get("b").map(String::as_str), Some("2"));
    }
}
