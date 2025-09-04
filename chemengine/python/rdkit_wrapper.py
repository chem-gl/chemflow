from rdkit import Chem
from rdkit.Chem import Descriptors


def mol_from_smiles(smiles: str) -> Chem.Mol:
    mol = Chem.MolFromSmiles(smiles)
    if mol is None:
        raise ValueError(f"Mol inválido: {smiles}")
    return mol


def mol_weight(smiles: str) -> float:
    mol = mol_from_smiles(smiles)
    descriptor = Descriptors.MolWt(mol)
    return float(descriptor)
