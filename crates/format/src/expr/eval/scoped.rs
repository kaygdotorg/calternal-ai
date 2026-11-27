//! Authorized list candidates and adjacent civil period scopes.

use super::{Diagnostic, Evaluator, Node, NodeKind, Row, Site, Value};

const MATCH_LIMIT: &str = "Too many items matched the formula";
const CANDIDATE_LIMIT: &str = "Too many items to test";

pub(super) fn too_many_matches(site: Site) -> Diagnostic {
    site.error(MATCH_LIMIT)
}

pub(crate) fn is_list_limit(error: &Diagnostic) -> bool {
    matches!(error.message.as_str(), MATCH_LIMIT | CANDIDATE_LIMIT)
}

impl<'a> Evaluator<'a, '_> {
    pub(super) fn all(&mut self, args: &[Node], site: Site) -> Result<Value, Diagnostic> {
        if !self.named_total {
            return Err(site.error("Use all() only in a named total"));
        }
        let key = reference_key(&args[0]).expect("checked kind selector");
        let snapshots = self.snapshot.and_then(|snapshot| snapshot.kinds);
        let kinds = if self.historical {
            snapshots
        } else {
            snapshots.or(self.context.kinds)
        };
        let rows = kinds.and_then(|kinds| kinds.get(&key)).ok_or_else(|| {
            site.error(format!(
                "No authorized whole-kind rows were supplied for {key}"
            ))
        })?;
        self.filter_rows(rows, args.get(1), site).map(Value::List)
    }

    pub(super) fn previous(&mut self, node: &Node, site: Site) -> Result<Value, Diagnostic> {
        let Some(period) = self.period else {
            return Err(site.error("Use previous() only in a per-period formula"));
        };
        let Some(snapshot) = self.snapshot else {
            return Ok(Value::Empty);
        };
        let Some(previous) = snapshot
            .previous
            .filter(|previous| period.precedes(previous.date, snapshot.date))
        else {
            return Ok(Value::Empty);
        };
        let old_snapshot = self.snapshot.replace(previous);
        let old_historical = std::mem::replace(&mut self.historical, true);
        let old_candidate = self.candidate.take();
        let old_context = std::mem::replace(
            &mut self.context,
            super::EvalContext {
                today: Some(previous.date),
                ..super::EvalContext::default()
            },
        );
        let result = self.eval(node, None);
        self.context = old_context;
        self.snapshot = old_snapshot;
        self.historical = old_historical;
        self.candidate = old_candidate;
        match result {
            Err(error) if is_list_limit(&error) => Ok(Value::Empty),
            result => result,
        }
    }

    pub(super) fn reverse(
        &mut self,
        args: &[Node],
        outer: Option<&Value>,
        site: Site,
    ) -> Result<Value, Diagnostic> {
        if outer.is_some() || self.candidate.is_some() || self.hops != 0 {
            return Err(site.error("A formula follows at most one link"));
        }
        let key = reference_key(&args[0])
            .ok_or_else(|| site.error("Name a kind and its link field in from"))?;
        let predicate = args.get(1).filter(
            |node| !matches!(&node.kind, NodeKind::Name(name) if name == "ignoring default"),
        );
        let reverse = if self.historical {
            Some(
                self.snapshot
                    .and_then(|snapshot| snapshot.reverse)
                    .ok_or_else(|| self.historical_input(site))?,
            )
        } else {
            self.context.reverse
        };
        let candidates = reverse
            .and_then(|reverse| reverse.get(&key))
            .map_or(&[][..], Vec::as_slice);
        self.hops += 1;
        let values = self.filter_rows(candidates, predicate, site);
        self.hops -= 1;
        values.map(Value::List)
    }

    fn filter_rows(
        &mut self,
        rows: &[Row<'a>],
        predicate: Option<&Node>,
        site: Site,
    ) -> Result<Vec<Value>, Diagnostic> {
        if rows.len() > 10_000 {
            return Err(site.error(CANDIDATE_LIMIT));
        }
        self.charge(rows.len(), site)?;
        let mut output = Vec::new();
        for &row in rows {
            if let Some(condition) = predicate {
                let old = self.candidate.replace(row);
                let passes = self.eval(condition, None);
                self.candidate = old;
                if !passes?.truth() {
                    continue;
                }
            }
            if output.len() == 1_000 {
                return Err(too_many_matches(site));
            }
            output.push(self.clone_row(Some(row), site)?);
        }
        Ok(output)
    }

    pub(super) fn filter(
        &mut self,
        values: Vec<Value>,
        predicate: Option<&Node>,
        site: Site,
        limit: usize,
    ) -> Result<Vec<Value>, Diagnostic> {
        if values.len() > 10_000 {
            return Err(site.error(CANDIDATE_LIMIT));
        }
        self.charge(values.len(), site)?;
        let mut output = Vec::new();
        for value in values {
            if let Some(condition) = predicate {
                let linked;
                let scope = match &value {
                    Value::Text(key) => {
                        linked = self.clone_row(self.link(key, site)?, site)?;
                        &linked
                    }
                    _ => &value,
                };
                let previous = self.hops;
                self.hops += usize::from(matches!(value, Value::Text(_)));
                let passes = self.eval(condition, Some(scope));
                self.hops = previous;
                if !passes?.truth() {
                    continue;
                }
            }
            if output.len() == limit {
                return Err(too_many_matches(site));
            }
            output.push(value);
        }
        Ok(output)
    }
}

pub(crate) fn reference_key(node: &Node) -> Option<String> {
    match &node.kind {
        NodeKind::Name(name) => Some(name.clone()),
        NodeKind::Member(base, field) => Some(format!("{}.{}", reference_key(base)?, field)),
        _ => None,
    }
}
