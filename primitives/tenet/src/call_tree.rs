use ethereum_types::{H160, H256};
use lazy_static::lazy_static;
use parking_lot::Mutex;
use tokio::runtime::Runtime;
use std::collections::HashMap;
use crate::model::PoM;
use serde::{Serialize, Deserialize};
use serde_json;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodeInfo {
    challenge_id: H256,
    root_id: H256,
    timeout: u64,
    caller: H160,
    callee: Option<H160>,
    call_depth: u64,
    state: crate::fsm::State,
    pom: Option<PoM>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Node {
    node_info: NodeInfo,
    children: Vec<Node>,
}

impl Node {
    fn new() -> Self {
        Node {
            node_info: NodeInfo { 
                challenge_id: H256::zero(), 
                root_id: H256::zero(), 
                timeout: 0, 
                caller: H160::zero(), 
                callee: None, 
                call_depth: 0,
                state: crate::fsm::State::Default,
                pom: None,
            },
            children: Vec::new(),
        }
    }
    
    fn insert(&mut self, node_info: NodeInfo) {
        let new_node_info = NodeInfo { 
            challenge_id: node_info.challenge_id,
            root_id: node_info.root_id,
            timeout: node_info.timeout,
            caller: node_info.caller,
            callee: node_info.callee,
            call_depth: node_info.call_depth,
            state: node_info.state,
            pom: node_info.pom,
        };

        let new_node = Node {
            node_info: new_node_info,
            children: Vec::new(),
        };

        self.children.push(new_node);
    }

    fn search(&self, challenge_id: H256, depth: u64) -> Option<(&Node, u64)> {
        if self.node_info.challenge_id == challenge_id {
            return Some((self, depth));
        }

        for child in &self.children {
            if let Some(found_node) = child.search(challenge_id, depth + 1) {
                return Some(found_node);
            }
        }

        None
    }

    fn search_and_insert(&mut self, pom: PoM, depth: u64) -> Option<(&Node, u64)> {
        if self.node_info.challenge_id == pom.challenge_id {
            self.insert(NodeInfo {
                challenge_id: pom.challenge_id, 
                root_id: pom.root_id, 
                timeout: pom.timeout,
                caller: pom.caller, 
                callee: pom.callee, 
                call_depth: depth + 1,
                state: pom.state.clone(),
                pom: Some(pom.clone())
               });
            return Some((self, depth));
        }

        for child in &mut self.children {
            if let Some(found_node)  = 
                child.search_and_insert(pom.clone(), depth + 1) {
                    return Some(found_node);
            }
        }
        None
    }

    fn search_and_update(
        &mut self, challenge_id: H256,
         from_state: crate::fsm::State, 
         to_state: crate::fsm::State,
          depth: u64
    ) -> bool {
        let mut updated = false;
        if self.node_info.challenge_id == challenge_id {
            if self.node_info.state == from_state {
                self.node_info.state = to_state.clone();
                updated = true;
            }
            return updated;
        }

        for child in &mut self.children {
             child.search_and_update(
                challenge_id, from_state.clone(), to_state.clone(), depth + 1
            );
        }

        panic!("ERROR: Call Tree not found caller when updating node");
    }

    fn search_by_addr(&self, callee: H160, depth: u64) -> Option<(&Node, u64)> {
        if self.node_info.callee.unwrap_or_default() == callee {
            return Some((self, depth));
        }

        for child in &self.children {
            if let Some(found_node) = child.search_by_addr(callee, depth + 1) {
                return Some(found_node);
            }
        }

        None
    }

    fn unfreeze(&mut self) -> bool {
        let mut all_children_responsed = true;
        for child in &self.children {
            if child.node_info.state != crate::fsm::State::Responsed {
                all_children_responsed = false;
            }
        }
        if all_children_responsed {
            self.node_info.timeout = 12;
            self.node_info.state = crate::fsm::State::Challenging;
            self.node_info.pom.as_mut().unwrap().timeout = 12;
            self.node_info.pom.as_mut().unwrap().state = crate::fsm::State::Challenging;
            return true;
        }

        false
    }

    fn check_timeout(&mut self, block_number: u64) -> Option<&Node> {
        if self.node_info.timeout <= block_number {
            self.node_info.state = crate::fsm::State::Timeout;
            self.node_info.pom.as_mut().unwrap().state = crate::fsm::State::Timeout;
            return Some(self);
        }
        for child in &mut self.children {
            child.check_timeout(block_number);
        }
        None
    }

    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    fn from_json(json_string: &str) -> Node {
        serde_json::from_str(json_string).unwrap()
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert() {
        let mut root_node = Node {
            node_info: NodeInfo {
                challenge_id: H256::random(),
                root_id: H256::random(),
                timeout: 6,
                caller: H160::random(),
                callee: Some(H160::random()),
                call_depth: 0,
                state: crate::fsm::State::Default,
                pom: None,
            },
            children: Vec::new(),
        };

        let test_node_info2 = NodeInfo {
            challenge_id: H256::random(),
            root_id: H256::random(),
            timeout: 8,
            caller: H160::random(),
            callee: Some(H160::random()),
            call_depth: 1,
            state: crate::fsm::State::Default,
            pom: None,
        };
        let new_node_info = NodeInfo {
            challenge_id: test_node_info2.challenge_id,
            root_id: test_node_info2.root_id,
            timeout: test_node_info2.timeout,
            caller: test_node_info2.caller,
            callee: test_node_info2.callee,
            call_depth: test_node_info2.call_depth,
            state: test_node_info2.state,
            pom: None,
        };

        root_node.insert(new_node_info.clone());

        assert_eq!(root_node.children.len(), 1);
        let inserted_node = &root_node.children[0];
        assert_eq!(inserted_node.node_info.challenge_id, test_node_info2.challenge_id);
        assert_eq!(inserted_node.node_info.root_id, test_node_info2.root_id);
        assert_eq!(inserted_node.node_info.timeout, test_node_info2.timeout);
        assert_eq!(inserted_node.node_info.caller, test_node_info2.caller);
        assert_eq!(inserted_node.node_info.callee, test_node_info2.callee);
        assert_eq!(inserted_node.node_info.call_depth, test_node_info2.call_depth);
    }

    #[test]
    fn test_serde() {
        let root_node = Node {
            node_info: NodeInfo {
                challenge_id: H256::random(),
                root_id: H256::random(),
                timeout: 8,
                caller: H160::random(),
                callee: Some(H160::random()),
                call_depth: 0,
                state: crate::fsm::State::Default,
                pom: None
            },
            children: Vec::new(),
        };

        let json_string = 
            serde_json::to_string(&root_node).expect("Failed to serialize to JSON");
        println!("Json String: , {:?}", json_string);

    }
}

lazy_static! {
    static ref CALL_TREE_MAP: Mutex<HashMap<H256, String>> = Mutex::new(HashMap::new());
}

pub fn cache_pom(pom: PoM) {
    let root_id = pom.root_id.clone();
    // Get relevant call tree
    let mut call_tree_map = CALL_TREE_MAP.lock();
    let mut root = Node::new();
    if let Some(value) = call_tree_map.get(&root_id) {
        root = Node::from_json(&value);
        root.search_and_insert(pom.clone(), 0);
    } else {
        root.node_info = NodeInfo {
            challenge_id: pom.clone().challenge_id, 
            root_id: pom.clone().root_id, 
            timeout: pom.clone().timeout,
            caller: pom.clone().caller, 
            callee: pom.clone().callee, 
            call_depth: 0,
            state: pom.clone().state,
            pom: Some(pom)
           };
    }

    // Cache call tree
    call_tree_map.insert(root_id, root.to_json());

    drop(call_tree_map);
}

pub fn check_start_challenge(pom: PoM) {
    let mut call_tree_map = CALL_TREE_MAP.lock();
    let mut root = Node::new();
    let mut updated = false;
    if let Some(value) = call_tree_map.get(&pom.root_id) {
        root = Node::from_json(&value);
        updated = root.search_and_update(
            pom.challenge_id, 
            crate::fsm::State::Default, 
            crate::fsm::State::Challenging, 
            0
        );
    }

    // Cache call tree
    call_tree_map.insert(pom.root_id, root.to_json());

    if updated {
        // send challenge to L1
        tokio::spawn(async {
            crate::l1_helper::update_challenge_bytes(
                String::from(""), pom, Vec::new()
            ).await.unwrap();
        });
    
    }

    drop(call_tree_map);
}

pub fn check_response(pom: PoM) -> bool{
    let mut call_tree_map = CALL_TREE_MAP.lock();
    let mut root = Node::new();
    let mut responsed = false;
    if let Some(value) = call_tree_map.get(&pom.root_id) {
        root = Node::from_json(&value);
        responsed = root.search_and_update(
            pom.challenge_id, 
            crate::fsm::State::Default, 
            crate::fsm::State::Responsed, 
            0
        );
    }

    // Cache call tree
    call_tree_map.insert(pom.root_id, root.to_json());

    drop(call_tree_map);
    responsed
}

pub fn handle_challenge(pom: PoM) {
    let mut call_tree_map = CALL_TREE_MAP.lock();
    let mut root = Node::new();
    let root_id = pom.root_id;
    if let Some(value) = call_tree_map.get(&root_id) {
        root = Node::from_json(&value);
        let challenge_id = pom.challenge_id;
        let mut challenged_depth = 0;
        if let Some((_, depth)) =  root.search_and_insert(pom.clone(), 0) {
            challenged_depth = depth;
        }

        if pom.callee.unwrap_or_default() == crate::config::TENET_NODE_ADDR {
            // for self node
            root.search_and_update(
                pom.challenge_id, 
                crate::fsm::State::Challenging, 
                crate::fsm::State::Responsed, 
                0
            );
            let mut response_pom = pom.clone();
            response_pom.state = crate::fsm::State::Responsed;
            tokio::spawn(async {
                crate::l1_helper::update_challenge_bytes(
                    String::from(""), pom, Vec::new()
                ).await.unwrap();
            });
        }
        
        if let Some((node, _)) = 
                root.search_by_addr(crate::config::TENET_NODE_ADDR, 0) {
            if challenged_depth > node.node_info.call_depth {
                // for deeper node
                root.search_and_update(
                    node.node_info.challenge_id, 
                    crate::fsm::State::Default, 
                    crate::fsm::State::Frozen, 
                    0
                );
            } else {
                // for other node
                root.search_and_update(
                    challenge_id, 
                    crate::fsm::State::Default, 
                    crate::fsm::State::Challenging, 
                    0
                );

            }
        }
    }
    // Cache call tree
    call_tree_map.insert(root_id, root.to_json());

    drop(call_tree_map);
}

pub fn handle_response(pom: PoM) {
    let mut call_tree_map = CALL_TREE_MAP.lock();
    let mut root = Node::new();
    let root_id = pom.root_id;
    if let Some(value) = call_tree_map.get(&root_id) {
        let challenge_id = pom.challenge_id;
        root = Node::from_json(&value);
        root.search_and_update(
            challenge_id, 
            crate::fsm::State::Challenging, 
            crate::fsm::State::Responsed, 
            0
        );
        // check unfreeze
        if let Some((node, _)) = 
            root.search_by_addr(crate::config::TENET_NODE_ADDR, 0) {
            let mut mut_node = node.clone();
            let unfrozen = mut_node.unfreeze();
            let unfrozen_pom = mut_node.node_info.pom.unwrap();
            if unfrozen {
                root.search_and_insert(unfrozen_pom.clone(), 0);
                tokio::spawn(async move {
                    crate::l1_helper::update_challenge_bytes(
                    String::from(""), unfrozen_pom.clone(), Vec::new()
                    ).await.unwrap();
                });
            }
        }
    }

    // Cache call tree
    call_tree_map.insert(root_id, root.to_json());

    drop(call_tree_map);
}

pub fn check_timeout_and_punish(pom: PoM) {
    let mut call_tree_map = CALL_TREE_MAP.lock();
    let mut root = Node::new();
    let root_id = pom.root_id;
    if let Some(value) = call_tree_map.get(&root_id) {
        root = Node::from_json(&value);
        let block_number = 
            Runtime::new().unwrap().block_on(crate::l1_helper::get_block_number());
        if let Some(node) = root.check_timeout(block_number) {
            // punish
            println!("Punish {:?}", node.node_info.challenge_id);
        }
    }

    // Cache call tree
    call_tree_map.insert(root_id, root.to_json());

    drop(call_tree_map);
}