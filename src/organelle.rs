//! ledger-graph as an organelle (Tier A native migration).
//!
//! Two ReadOnly ops over a ledger JSONL file:
//!   parse {path?}        -> index stats (entries, parse_ms, actors, types, chats, seq_indexed, ref_edges)
//!   view  {name, path?}  -> one graph view (claims|actors|domains|verdicts|contradictions)
//!
//! Execution personality: fs read-only, allowlist = the single ledger path
//! supplied per call (default ~/ledger/ledger-0001.jsonl). No network, no
//! writes, no secrets. Stateless: suspend = no-op, kill = drop in-flight.
//! Resource envelope: 1 cpu, 512 MB, 60 s wall-time.

use organelle_host::{CallContext, HostRef, Organelle, OrganelleError};
use organelle_manifest::{
    Authority, Contract, MutationClass, OrganelleManifest, ReceiptPolicy, Transport, Valence,
    MANIFEST_SCHEMA_VERSION, RECEIPT_ALGORITHM,
};
use serde_json::Value;
use std::sync::Arc;

use crate::ledger::Ledger;
use crate::views;

fn default_path() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/home/hatch".to_string())
        + "/ledger/ledger-0001.jsonl"
}

fn manifest() -> OrganelleManifest {
    let path_schema = serde_json::json!({
        "type": "object",
        "properties": {"path": {"type": "string"}}
    });
    let view_schema = serde_json::json!({
        "type": "object",
        "required": ["name"],
        "properties": {
            "name": {"type": "string"},
            "path": {"type": "string"}
        }
    });
    OrganelleManifest {
        schema_version: MANIFEST_SCHEMA_VERSION.to_string(),
        name: "ledger-graph".to_string(),
        codon: "lg".to_string(),
        version: "0.2.0".to_string(),
        description: "fast ledger parser + on-demand graph views (organelle)".to_string(),
        contracts: vec![
            Contract {
                op: "parse".to_string(),
                description: "parse ledger JSONL, return index stats".to_string(),
                input_schema: path_schema,
                mutation_class: None, // organelle-level ReadOnly
                required_capability: None,
            },
            Contract {
                op: "view".to_string(),
                description: "materialize a graph view: claims|actors|domains|verdicts|contradictions"
                    .to_string(),
                input_schema: view_schema,
                mutation_class: None,
                required_capability: None,
            },
        ],
        authority: Authority {
            mutation_class: MutationClass::ReadOnly,
            declared_valence: Valence::Proven, // deterministic parse, recomputable
            required_capability: None,
            granted_capabilities: vec![],
        },
        transports: Transport::all().to_vec(),
        receipt_policy: ReceiptPolicy {
            content_addressed: true,
            algorithm: RECEIPT_ALGORITHM.to_string(),
            chain: true,
        },
    }
}

pub struct LedgerGraph {
    manifest: OrganelleManifest,
}

pub fn organelle() -> Arc<dyn Organelle> {
    Arc::new(LedgerGraph { manifest: manifest() })
}

fn load_ledger(input: &Value) -> Result<Ledger, OrganelleError> {
    let path = input
        .get("path")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(default_path);
    Ledger::load(&path)
        .map_err(|e| OrganelleError::failed(format!("cannot load ledger at {path}: {e}")))
}

impl Organelle for LedgerGraph {
    fn manifest(&self) -> OrganelleManifest {
        self.manifest.clone()
    }

    fn handle(
        &self,
        _ctx: &CallContext,
        _host: &dyn HostRef,
        op: &str,
        input: Value,
    ) -> Result<Value, OrganelleError> {
        match op {
            "parse" => {
                let l = load_ledger(&input)?;
                let mut types: Vec<(&String, usize)> =
                    l.by_type.iter().map(|(k, v)| (k, v.len())).collect();
                types.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
                let types_obj: serde_json::Map<String, Value> = types
                    .into_iter()
                    .map(|(k, c)| (k.clone(), Value::from(c as u64)))
                    .collect();
                Ok(serde_json::json!({
                    "entries": l.len(),
                    "parse_ms": l.parse_ms,
                    "actors": l.by_actor.len(),
                    "types": types_obj,
                    "chats": l.by_chat.len(),
                    "seq_indexed": l.by_seq.len(),
                    "ref_edges": l.referenced_by.values().map(|v| v.len()).sum::<usize>(),
                }))
            }
            "view" => {
                let name = input
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| OrganelleError::failed("missing required input: name"))?;
                let l = load_ledger(&input)?;
                let out = match name {
                    "claims" => views::view_claims(&l),
                    "actors" => views::view_actors(&l),
                    "domains" => views::view_domains(&l),
                    "verdicts" => views::view_verdicts(&l),
                    "contradictions" => views::view_contradictions(&l),
                    _ => {
                        return Err(OrganelleError::not_found(format!(
                            "unknown view '{name}'. names: claims|actors|domains|verdicts|contradictions"
                        )))
                    }
                };
                serde_json::from_str(&out)
                    .map_err(|e| OrganelleError::failed(format!("view output is not JSON: {e}")))
            }
            _ => Err(OrganelleError::not_found(format!("unknown op {op}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use organelle_host::Host;
    use organelle_manifest::validate_manifest_json;

    fn manifest_json() -> Value {
        let txt = include_str!("../organelles/ledger-graph.manifest.json");
        serde_json::from_str(txt).expect("manifest JSON parses")
    }

    #[test]
    fn static_manifest_validates() {
        let v = manifest_json();
        let m = validate_manifest_json(&v).expect("static manifest must validate");
        assert_eq!(m.name, "ledger-graph");
        assert_eq!(m.codon, "lg");
    }

    #[test]
    fn rust_manifest_matches_static_json() {
        let from_rust = serde_json::to_value(manifest()).unwrap();
        let from_file = manifest_json();
        assert_eq!(from_rust, from_file, "Rust manifest() and organelles/*.json must agree");
    }

    #[test]
    fn registers_and_serves_through_host_with_receipts() {
        let host = Host::new();
        host.register(organelle()).expect("registration must succeed");
        let ctx = CallContext::new("pilot-test", vec![]);

        // parse against the real ledger (read-only)
        let stats = host
            .call(&ctx, "lg", "parse", serde_json::json!({}))
            .expect("parse call");
        assert!(stats.get("entries").and_then(|v| v.as_u64()).unwrap_or(0) > 0);

        // view one graph
        let actors = host
            .call(&ctx, "lg", "view", serde_json::json!({"name": "actors"}))
            .expect("view call");
        assert!(actors.is_object() || actors.is_array());

        // unknown view -> clean failure, still receipted
        assert!(host
            .call(&ctx, "lg", "view", serde_json::json!({"name": "bogus"}))
            .is_err());

        // every path emitted a receipt; chain verifies
        let n = host.verify_chain().expect("receipt chain must verify");
        assert!(n >= 4, "register + 3 calls = >=4 receipts, got {n}");
    }
}
