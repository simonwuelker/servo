use std::net::TcpStream;

use serde_json::{json, Map, Value};

use crate::actor::{ActorMessageStatus, ActorRegistry};
use crate::protocol::JsonPacketStream;
use crate::{Actor, StreamId};

/// Computes issues with CSS declarations
///
/// Gecko implementation: <https://searchfox.org/mozilla-central/source/devtools/server/actors/compatibility/compatibility.js>
/// Specification: <https://searchfox.org/mozilla-central/source/devtools/shared/specs/compatibility.js>
pub struct CompatibilityActor {
    pub name: String,
}

impl Actor for CompatibilityActor {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn handle_message(
        &self,
        _registry: &ActorRegistry,
        msg_type: &str,
        msg: &Map<String, Value>,
        stream: &mut TcpStream,
        _id: StreamId,
    ) -> Result<ActorMessageStatus, ()> {
        let status = match msg_type {
            "getCSSDeclarationBlockIssues" => {
                let Some(dom_rules_declarations) = msg.get("domRulesDeclarations") else {
                    // TODO: Send missing parameter error
                    log::warn!("Missing parameter in for \"getCSSDeclarationBlockIssues\"");
                    return Ok(ActorMessageStatus::Ignored);
                };

                let Some(dom_rules_declarations_list) = dom_rules_declarations.as_array() else {
                    // TODO: Send invalid parameter error
                    log::warn!("Invalid parameter in for \"getCSSDeclarationBlockIssues\"");
                    return Ok(ActorMessageStatus::Ignored);
                };

                // TODO: Actually compute issues
                let compatibility_issues =
                    vec![Vec::<()>::default(); dom_rules_declarations_list.len()];

                let msg = json!({
                    "from": &self.name,
                    "compatibilityIssues": compatibility_issues,
                });
                let _ = stream.write_json_packet(&msg);

                ActorMessageStatus::Processed
            },
            _ => ActorMessageStatus::Ignored,
        };

        Ok(status)
    }
}
