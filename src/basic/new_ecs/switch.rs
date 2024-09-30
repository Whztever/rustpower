use crate::io::pandapower::SwitchType;
use bevy_ecs::prelude::*;
use derive_more::{Deref, DerefMut};
use nalgebra::{vector, Complex};
use nalgebra_sparse::CooMatrix;
use std::collections::{HashMap, HashSet};

use super::elements::*;

/// Represents a switch in the network.
#[derive(Default, Debug, Clone, Component)]
pub struct Switch {
    pub bus: i64,
    pub element: i64,
    pub et: SwitchType,
    pub z_ohm: f64,
}

/// Represents a switch state in the network.
#[derive(Default, Debug, Clone, Component, Deref, DerefMut)]
pub struct SwitchState(pub bool);

/// Represents merging two nodes in the network.
#[derive(Default, Debug, Clone, Component)]
pub struct MergeNode(pub usize, pub usize);

/// A union-find (disjoint set) structure for merging nodes.
#[derive(Default, Debug, Clone)]
pub struct NodeMerge {
    pub parent: HashMap<u64, u64>,
    pub rank: HashMap<u64, u64>,
}

/// A mapping from old nodes to new nodes after merging, stored as a resource.
#[derive(Default, Debug, Clone, Deref, DerefMut, Resource)]
pub struct NodeMapping(HashMap<u64, u64>);

impl NodeMerge {
    /// Creates a new union-find (disjoint set) structure for the given nodes.
    pub fn new(nodes: &[u64]) -> Self {
        let mut parent = HashMap::new();
        let mut rank = HashMap::new();
        for &node in nodes {
            parent.insert(node, node);
            rank.insert(node, 0);
        }
        NodeMerge { parent, rank }
    }

    /// Finds the root of the node, with path compression.
    fn find(&mut self, node: u64) -> u64 {
        let mut root = node;

        while self.parent[&root] != root {
            root = self.parent[&root];
        }

        let mut current = node;
        while self.parent[&current] != root {
            let parent = self.parent[&current];
            self.parent.insert(current, root);
            current = parent;
        }
        root
    }

    /// Unites two nodes by their roots.
    pub fn union(&mut self, node1: u64, node2: u64) {
        let root1 = self.find(node1);
        let root2 = self.find(node2);
        if root1 != root2 {
            let rank1 = self.rank[&root1];
            let rank2 = self.rank[&root2];
            if rank1 < rank2 {
                self.parent.insert(root1, root2);
            } else {
                self.parent.insert(root2, root1);
                if rank1 == rank2 {
                    *self.rank.get_mut(&root1).unwrap() += 1;
                }
            }
        }
    }

    /// Generates a node mapping based on union-find results, starting with a given index.
    pub fn get_node_mapping(&self, starting_idx: u64) -> HashMap<u64, u64> {
        let mut root_to_new_id = HashMap::new();
        let mut node_mapping = HashMap::new();
        let mut new_node_id = starting_idx;
        let mut nodes: Vec<_> = self.parent.keys().collect();
        nodes.sort();
        for &node in &nodes {
            let root = self.parent.get(&(*node as u64)).unwrap();
            if !root_to_new_id.contains_key(&root) {
                root_to_new_id.insert(root, new_node_id);
                new_node_id += 1;
            }
            node_mapping.insert(*node, root_to_new_id[&root]);
        }
        node_mapping
    }
}

/// Processes the state of switches and updates network components accordingly.
#[allow(dead_code)]
pub fn process_switch_state(
    mut cmd: Commands,
    nodes: Res<NodeLookup>,
    net: Res<PPNetwork>,
    q: Query<(Entity, &Switch, &SwitchState)>,
) {
    let node_idx: Vec<u64> = nodes.0.keys().map(|x| *x as u64).collect();
    let union_find: Option<NodeMerge> = if q.iter().len() > 0 {
        Some(NodeMerge::new(&node_idx))
    } else {
        None
    };

    q.iter().for_each(|(entity, switch, closed)| {
        let _z_ohm = switch.z_ohm;

        match switch.et {
            SwitchType::SwitchBusLine => todo!(),
            SwitchType::SwitchBusTransformer => todo!(),
            SwitchType::SwitchTwoBuses => {
                let (node1, node2) = (switch.bus, switch.element);
                if **closed {
                    if _z_ohm == 0.0 {
                        let v_base = net.bus[switch.bus as usize].vn_kv;
                        cmd.entity(entity).insert(AdmittanceBranch {
                            y: Admittance(Complex::new(1e6, 0.0)),
                            port: Port2(vector![node1, node2]),
                            v_base: VBase(v_base),
                        });
                    } else {
                        let v_base = net.bus[switch.bus as usize].vn_kv;
                        cmd.entity(entity).insert(AdmittanceBranch {
                            y: Admittance(Complex::new(_z_ohm, 0.0)),
                            port: Port2(vector![node1, node2]),
                            v_base: VBase(v_base),
                        });
                    }
                }
            }
            SwitchType::SwitchBusTransformer3w | SwitchType::Unknown => {}
        }
    });

    if union_find.is_some() {
        cmd.insert_resource(NodeMapping(union_find.unwrap().get_node_mapping(0)));
    }
}

/// Placeholder function for future node merge or split logic.
#[allow(dead_code)]
pub fn node_merge_split(_cmd: Commands, _nodes: Res<NodeMapping>) {}
#[allow(dead_code)]
/// Builds an aggregation matrix based on the provided nodes and node mapping.
fn build_aggregation_matrix(nodes: &[u64], node_mapping: &HashMap<u64, u64>) -> CooMatrix<u64> {
    let original_node_count = nodes.len();
    let new_node_count = node_mapping.values().collect::<HashSet<_>>().len();
    let mut mat = CooMatrix::new(original_node_count, new_node_count);
    mat.push(0, 0, 1);
    todo!()
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use std::{env, fs};

    use serde_json::{Map, Value};

    use crate::{
        basic::new_ecs::{network::*, post_processing::PostProcessing},
        io::pandapower::{load_pandapower_json, load_pandapower_json_obj},
    };

    use super::*;
    
    /// Loads a JSON object from a string.
    fn load_json_from_str(file_content: &str) -> Result<Map<String, Value>, std::io::Error> {
        let parsed: Value = serde_json::from_str(&file_content)?;
        let obj: Map<String, Value> = parsed.as_object().unwrap().clone();
        Ok(obj)
    }

    /// Loads a JSON object from a file.
    fn load_json(file_path: &str) -> Result<Map<String, Value>, std::io::Error> {
        let file_content = fs::read_to_string(file_path)
            .expect("Error reading network file");
        let obj = load_json_from_str(&file_content);
        obj
    }

    #[test]
    /// Tests the node merging logic using union-find (disjoint set).
    fn test_node_merge() {
        let nodes = vec![1, 2, 3, 4, 5, 6, 7];
        let switches = vec![
            Switch {
                bus: 2,
                element: 3,
                et: SwitchType::SwitchTwoBuses,
                z_ohm: 0.0,
            },
            Switch {
                bus: 3,
                element: 4,
                et: SwitchType::SwitchTwoBuses,
                z_ohm: 0.0,
            },
            Switch {
                bus: 5,
                element: 6,
                et: SwitchType::SwitchTwoBuses,
                z_ohm: 0.0,
            },
            Switch {
                bus: 6,
                element: 7,
                et: SwitchType::SwitchTwoBuses,
                z_ohm: 0.0,
            },
        ];

        let switch_states = vec![
            SwitchState(true),
            SwitchState(true),
            SwitchState(false),
            SwitchState(true),
        ];

        let mut uf = NodeMerge::new(&nodes);

        for (switch, state) in switches.iter().zip(switch_states.iter()) {
            if **state {
                if switch.et == SwitchType::SwitchTwoBuses {
                    uf.union(switch.bus as u64, switch.element as u64);
                }
            }
        }

        assert_eq!(uf.find(2), uf.find(3));
        assert_eq!(uf.find(3), uf.find(4));
        assert_ne!(uf.find(5), uf.find(6));
        assert_eq!(uf.find(6), uf.find(7));
    }

    #[test]
    /// Tests the entire power flow ECS system, including switch processing.
    fn test_ecs_switch() {
        let dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let folder = format!("{}/cases/test/", dir);
        let name = folder.to_owned() + "/new_input_PFLV_modified.json";
        let json = load_json(&name).unwrap();
        let json: Map<String, Value> = json
            .get("pp_network")
            .and_then(|v| v.as_object())
            .unwrap()
            .clone();
        let net = load_pandapower_json_obj(&json);
        let mut pf_net = PowerGrid::default();
        pf_net.world_mut().insert_resource(PPNetwork(net));
        pf_net.init_pf_net();
        let node_mapping = pf_net.world().get_resource::<NodeMapping>().unwrap();
        let mut nodes: Vec<_> = node_mapping.keys().collect();
        nodes.sort();

        pf_net.run_pf();
        pf_net.post_process();
        pf_net.print_res_bus();
        assert_eq!(
            pf_net
                .world()
                .get_resource::<PowerFlowResult>()
                .unwrap()
                .converged,
            true
        );
    }

    #[test]
    /// Tests the power flow calculation and generation of aggregation matrix.
    fn test_ecs_pf_switch() {
        let dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let folder = format!("{}/cases/test/", dir);
        let name = folder.to_owned() + "/new_input_PFLV_modified.json";
        let json = load_json(&name).unwrap();
        let json: Map<String, Value> = json
            .get("pp_network")
            .and_then(|v| v.as_object())
            .unwrap()
            .clone();
        let net = load_pandapower_json_obj(&json);
        let mut pf_net = PowerGrid::default();
        pf_net.world_mut().insert_resource(PPNetwork(net));
        pf_net.init_pf_net();
        let node_mapping = pf_net.world().get_resource::<NodeMapping>().unwrap();
        let mut nodes: Vec<u64> = node_mapping.keys().map(|x| *x).collect();
        nodes.sort();

        // let p_matrix = build_aggregation_matrix(nodes.as_slice(), &node_mapping.0);
        // println!("\nAggregation Matrix P:\n{:?}", p_matrix);
    }
}
