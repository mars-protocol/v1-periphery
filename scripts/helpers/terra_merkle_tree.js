import keccak256 from 'keccak256';
import { MerkleTree } from 'merkletreejs';

export class Terra_Merkle_Tree {

  constructor(accounts) {
    let leaves = accounts.map((a) => keccak256(a.address + a.amount  ));
    leaves.sort();
    this.tree = new MerkleTree(leaves, keccak256, { sort: true });
  }

  getMerkleTree() {
    return this.tree;
  }

  getMerkleRoot() {
    return this.tree.getHexRoot().replace('0x', '');
  }

  getMerkleProof(account) {
    return this.tree.getHexProof(keccak256(  account.address + account.amount  )) .map((v) => v.replace('0x', ''));
  }

  verify( proof, account) {
    let leaf_terra = keccak256(account.address + account.amount);
    let is_valid = this.tree.verify(proof, leaf_terra, this.tree.getHexRoot());
    return is_valid;
  }

}
