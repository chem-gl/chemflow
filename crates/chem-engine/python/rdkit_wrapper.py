from rdkit import Chem
from rdkit.Chem import Descriptors, inchi


def molecule_info(smiles: str) -> dict:
    mol = Chem.MolFromSmiles(smiles)
    if mol is None:
        raise ValueError("SMILES inválido")

    info = {
        "smiles": Chem.MolToSmiles(mol),
        "inchi": inchi.MolToInchi(mol),
        "inchikey": inchi.MolToInchiKey(mol),
        "num_atoms": mol.GetNumAtoms(),
        "mol_weight": Descriptors.MolWt(mol),
        "mol_formula": Chem.rdMolDescriptors.CalcMolFormula(mol)
    }
    return info
