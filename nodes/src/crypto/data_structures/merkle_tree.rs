use crate::crypto::crypto_lib::evaluation_domain::smallest_power_of_2_greater_or_eq_than;
use merkletree::{hash::Algorithm, merkle::MerkleTree, proof::Proof, store::VecStore};
use serde::{Deserialize, Serialize};
use sha2::{
    digest::{consts::U0, typenum::Unsigned},
    Digest, Sha256,
};
use std::hash::Hasher;
use std::marker::PhantomData;

type Bytes = Vec<u8>;
type U = U0;

pub type MHash = [u8; 32];
pub type MProof = Proof<MHash>;

#[derive(Clone, Default)]
struct MAlgorithm {
    hasher: Sha256,
    _marker: PhantomData<[u8; 32]>,
}

// Implémentation du trait Hasher pour Sha256Algorithm
impl Hasher for MAlgorithm {
    fn finish(&self) -> u64 {
        0
    }

    fn write(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
    }
}

// Implémentation du trait Algorithm pour Sha256Algorithm
impl Algorithm<[u8; 32]> for MAlgorithm {
    fn hash(&mut self) -> [u8; 32] {
        let hash_result = self.hasher.clone().finalize();
        let mut hash_array = [0u8; 32];
        hash_array.copy_from_slice(&hash_result);
        hash_array
    }

    fn reset(&mut self) {
        self.hasher = Sha256::new();
    }

    fn leaf(&mut self, leaf: [u8; 32]) -> [u8; 32] {
        self.hasher.update(leaf);
        self.hash()
    }

    fn node(&mut self, left: [u8; 32], right: [u8; 32], _height: usize) -> [u8; 32] {
        self.hasher.update(left);
        self.hasher.update(right);
        self.hash()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SerializableProof {
    sub_tree_proof: Option<Box<SerializableProof>>,
    top_layer_nodes: usize,
    sub_tree_layer_nodes: usize,
    lemma: Vec<MHash>,
    path: Vec<usize>,
}

impl SerializableProof {
    pub fn from_proof(proof: &MProof) -> Self {
        SerializableProof {
            sub_tree_proof: proof
                .sub_tree_proof
                .as_ref()
                .map(|sub_proof| Box::new(SerializableProof::from_proof(sub_proof))),
            top_layer_nodes: proof.top_layer_nodes(),
            sub_tree_layer_nodes: proof.sub_layer_nodes(),
            lemma: proof.lemma().clone(),
            path: proof.path().clone(),
        }
    }

    pub fn to_proof(&self) -> MProof {
        let res = Proof::<MHash>::new::<U, U>(
            self.sub_tree_proof
                .as_ref()
                .map(|sub_proof| Box::new(sub_proof.to_proof())),
            self.lemma.clone(),
            self.path.clone(),
        )
        .expect("Erreur lors de la conversion en Proof");
        assert!(res.sub_layer_nodes() == U::to_usize());
        res
    }
}

fn compute_tree(mut leafs: Vec<Bytes>) -> MerkleTree<MHash, MAlgorithm, VecStore<MHash>> {
    let p = smallest_power_of_2_greater_or_eq_than(leafs.len()).0;
    while leafs.len() < p {
        leafs.push(Vec::new())
    }
    MerkleTree::from_data(&leafs).unwrap()
}

pub fn compute_root(leafs: Vec<Bytes>) -> MHash {
    let mt = compute_tree(leafs);
    mt.root()
}

pub fn hash_leafs(leafs: Vec<Bytes>) -> Vec<MProof> {
    let n = leafs.len();
    let mt = compute_tree(leafs);
    (0..mt.leafs())
        .take(n)
        .map(|i| mt.gen_proof(i).unwrap())
        .collect()
}

pub fn verify(leaf: &Bytes, proof: &MProof) -> bool {
    proof.validate_with_data::<MAlgorithm>(leaf).unwrap()
}
