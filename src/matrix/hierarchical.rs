use super::*;
use log::{debug, info};
use rustc_hash::FxHashMap;

pub type ScopeId = i32;

#[derive(Debug, Clone)]
pub struct QVariableInfo {
    pub scope: ScopeId,
    pub is_universal: bool,
    pub copy_of: Variable,
}

impl VariableInfo for QVariableInfo {
    fn new() -> QVariableInfo {
        QVariableInfo {
            scope: -1,
            is_universal: false,
            copy_of: 0,
        }
    }
}

impl QVariableInfo {
    pub fn is_bound(&self) -> bool {
        self.scope >= 0
    }

    pub fn is_universal(&self) -> bool {
        debug_assert!(self.is_bound());
        self.scope % 2 == 1
    }

    pub fn is_existential(&self) -> bool {
        return !self.is_universal();
    }
}

#[derive(Debug)]
pub struct Scope {
    pub id: ScopeId,
    pub variables: Vec<Variable>,
}

impl Scope {
    fn new(id: ScopeId) -> Scope {
        Scope {
            id: id,
            variables: Vec::new(),
        }
    }

    pub fn contains(&self, variable: Variable) -> bool {
        self.variables
            .iter()
            .fold(false, |val, &var| val || var == variable)
    }
}

impl Dimacs for Scope {
    fn dimacs(&self) -> String {
        let mut dimacs = String::new();
        if self.id % 2 == 0 {
            dimacs.push_str("e ");
        } else {
            dimacs.push_str("a ");
        }
        for &variable in self.variables.iter() {
            dimacs.push_str(&format!("{} ", variable));
        }
        dimacs.push_str("0");
        dimacs
    }
}

#[derive(Debug)]
pub struct HierarchicalPrefix {
    variables: VariableStore<QVariableInfo>,
    pub scopes: Vec<Scope>,
}

#[derive(Eq, PartialEq)]
pub enum Quantifier {
    Existential,
    Universal,
}

impl Quantifier {
    pub fn swap(&self) -> Quantifier {
        match self {
            &Quantifier::Existential => Quantifier::Universal,
            &Quantifier::Universal => Quantifier::Existential,
        }
    }
}

impl From<usize> for Quantifier {
    fn from(item: usize) -> Self {
        if item % 2 == 0 {
            Quantifier::Existential
        } else {
            Quantifier::Universal
        }
    }
}

impl From<ScopeId> for Quantifier {
    fn from(item: ScopeId) -> Self {
        if item < 0 {
            panic!("scope id's have to be positive");
        }
        if item % 2 == 0 {
            Quantifier::Existential
        } else {
            Quantifier::Universal
        }
    }
}

impl Prefix for HierarchicalPrefix {
    type V = QVariableInfo;

    fn new(num_variables: usize) -> Self {
        HierarchicalPrefix {
            variables: VariableStore::new(num_variables),
            scopes: vec![Scope {
                id: 0,
                variables: Vec::new(),
            }],
        }
    }

    fn variables(&self) -> &VariableStore<Self::V> {
        &self.variables
    }

    fn import(&mut self, variable: Variable) {
        if !self.variables().get(variable).is_bound() {
            // bound free variables at top level existential quantifier
            self.add_variable(variable, 0);
        }
    }

    fn reduce_universal(&self, clause: &mut Clause) {
        clause.reduce_universal_qbf(self);
    }
}

impl HierarchicalPrefix {
    /// Creates a new scope with given quantification type
    pub fn new_scope(&mut self, quantifier: Quantifier) -> ScopeId {
        let last_scope: ScopeId = self.last_scope();
        if last_scope % 2 == quantifier as ScopeId {
            return last_scope;
        } else {
            self.scopes.push(Scope::new(last_scope + 1));
            return self.last_scope();
        }
    }

    /// Returns the last created scope
    pub fn last_scope(&self) -> ScopeId {
        debug_assert!(self.scopes.len() > 0);
        (self.scopes.len() - 1) as ScopeId
    }

    /// Adds a variable to a given scope
    ///
    /// Panics, if variable is already bound or scope does not exist (use new_scope first)
    pub fn add_variable(&mut self, variable: Variable, scope_id: ScopeId) {
        self.variables.import(variable);
        if self.variables.get(variable).is_bound() {
            panic!("variable cannot be bound twice");
        }
        if scope_id > self.last_scope() {
            panic!("scope does not exists");
        }
        let variable_info = self.variables.get_mut(variable);
        variable_info.scope = scope_id;
        variable_info.is_universal = scope_id % 2 == 1;
        let scope = &mut self.scopes[scope_id as usize];
        scope.variables.push(variable);
    }
}

impl Dimacs for HierarchicalPrefix {
    fn dimacs(&self) -> String {
        let mut dimacs = String::new();
        for ref scope in self.scopes.iter() {
            if scope.id == 0 && scope.variables.len() == 0 {
                continue;
            }
            dimacs.push_str(&format!("{}\n", scope.dimacs()));
        }
        dimacs
    }
}

#[derive(Debug)]
pub struct TreePrefix {
    variables: VariableStore<QVariableInfo>,
    pub roots: Vec<Box<ScopeNode>>,
}

#[derive(Debug)]
pub struct ScopeNode {
    pub scope: Scope,
    group: Variable,
    pub next: Vec<Box<ScopeNode>>,
}

impl Prefix for TreePrefix {
    type V = QVariableInfo;

    fn new(num_variables: usize) -> Self {
        TreePrefix {
            variables: VariableStore::new(num_variables),
            roots: Vec::new(),
        }
    }

    fn variables(&self) -> &VariableStore<Self::V> {
        &self.variables
    }

    fn import(&mut self, _variable: Variable) {
        panic!("not implemented");
    }

    fn reduce_universal(&self, _clause: &mut Clause) {
        panic!("not implemented");
    }
}

impl TreePrefix {
    fn to_hierarchical(&self) -> HierarchicalPrefix {
        let mut prefix = HierarchicalPrefix::new(self.variables.num_variables());
        for ref roots in self.roots.iter() {
            roots.to_hierarchical(0, &mut prefix);
        }
        prefix
    }
}

impl Dimacs for TreePrefix {
    fn dimacs(&self) -> String {
        // convert to hierarchical prefix and print that result
        self.to_hierarchical().dimacs()
    }
}

impl ScopeNode {
    fn to_hierarchical(&self, depth: usize, prefix: &mut HierarchicalPrefix) {
        if depth >= prefix.scopes.len() {
            prefix.new_scope(Quantifier::from(depth));
            debug_assert!(depth < prefix.scopes.len());
        }
        for &variable in self.scope.variables.iter() {
            prefix.add_variable(variable, depth as ScopeId);
        }
        for ref next in self.next.iter() {
            next.to_hierarchical(depth + 1, prefix);
        }
    }
}

impl Matrix<HierarchicalPrefix> {
    pub fn unprenex_by_miniscoping(
        matrix: Self,
        collapse_empty_scopes: bool,
    ) -> Matrix<TreePrefix> {
        let prefix = matrix.prefix;
        let mut variables = prefix.variables;
        let mut scopes = prefix.scopes;
        let mut clauses = matrix.clauses;
        let mut occurrences = matrix.occurrences;

        // we store for each variable the variable it is connected to
        // we compact this by using the smallest variable as characteristic element
        let mut partitions = Vec::with_capacity(variables.num_variables() + 1);
        for i in 0..variables.num_variables() + 1 {
            partitions.push(i as Variable);
        }

        let mut prev_scopes = Vec::new();
        let mut quantifier = Quantifier::Existential;
        while let Some(scope) = scopes.pop() {
            match quantifier {
                Quantifier::Existential => {
                    Self::union_over_connecting_sets(&clauses, &scope, &mut partitions, &variables);
                    prev_scopes = Self::partition_scopes(
                        scope,
                        &mut partitions,
                        &mut variables,
                        prev_scopes,
                        collapse_empty_scopes,
                    );
                }
                Quantifier::Universal => {
                    prev_scopes = Self::split_universal(
                        scope,
                        &partitions,
                        prev_scopes,
                        &mut variables,
                        &mut clauses,
                        &mut occurrences,
                    );
                }
            }

            quantifier = quantifier.swap();
        }

        let tree_prefix = TreePrefix {
            variables,
            roots: prev_scopes,
        };
        Matrix {
            prefix: tree_prefix,
            clauses: clauses,
            occurrences: occurrences,
            conflict: matrix.conflict,
            orig_clause_num: matrix.orig_clause_num,
        }
    }

    fn union_over_connecting_sets(
        clauses: &Vec<Clause>,
        scope: &Scope,
        partitions: &mut Vec<Variable>,
        variables: &VariableStore<QVariableInfo>,
    ) {
        for clause in clauses.iter() {
            let mut connection = None;
            for &literal in clause.iter() {
                let variable = literal.variable();
                let info = variables.get(variable);
                if !info.is_bound() {
                    continue;
                }
                if info.is_universal() {
                    continue;
                }
                if info.scope < scope.id {
                    continue;
                }

                let variable = variable as usize;

                // Check whether this variable connects some variable sets
                loop {
                    // Compacitify
                    let characteristic_elem = partitions[variable] as usize;
                    if partitions[characteristic_elem] != partitions[variable] {
                        partitions[variable] = partitions[characteristic_elem];
                    } else {
                        break;
                    }
                }

                match connection {
                    None => {
                        connection = Some(partitions[variable]);
                        continue;
                    }
                    Some(connecting_var) => {
                        if connecting_var < partitions[variable] {
                            // connection var is smaller, update variable and characteristic element
                            let characteristic_elem = partitions[variable] as usize;
                            partitions[characteristic_elem] = connecting_var;
                            partitions[variable] = connecting_var;
                        }
                        if connecting_var > partitions[variable] {
                            // connection var is greater, update connection var
                            partitions[connecting_var as usize] = partitions[variable];
                            connection = Some(partitions[variable]);
                        }
                    }
                }
            }
        }
        // last compactify
        for i in 1..partitions.len() {
            loop {
                let characteristic_elem = partitions[i] as usize;
                partitions[i] = partitions[characteristic_elem];
                let characteristic_elem = partitions[i] as usize;
                if partitions[i] < i as Variable || partitions[i] == partitions[characteristic_elem]
                {
                    break;
                }
            }
        }
    }

    fn partition_scopes(
        scope: Scope,
        partitions: &mut Vec<Variable>,
        variables: &mut VariableStore<QVariableInfo>,
        next: Vec<Box<ScopeNode>>,
        collapse_empty_scopes: bool,
    ) -> Vec<Box<ScopeNode>> {
        let mut scopes = Vec::new();

        let mut remaining_next = next;

        // maps characteristic variables to index of scopes vector
        let mut groups = FxHashMap::default();

        for i in 1..partitions.len() {
            let variable = i as Variable;
            {
                // we later access variables mutably
                let info = variables.get(variable);
                if !info.is_bound() {
                    continue;
                }
                if info.is_universal() {
                    continue;
                }
                if info.scope < scope.id {
                    continue;
                }
            }

            let partition = partitions[i];
            debug!("variable {} is in partition {}", i, partition);

            if partition == variable {
                // variable is chracteristic element of a variable group
                let mut node = Box::new(ScopeNode {
                    scope: Scope::new(scope.id),
                    group: partition,
                    next: Vec::new(),
                });
                if scope.contains(variable) {
                    node.scope.variables.push(variable);
                }

                // split next-scopes
                let mut j = 0;
                while j != remaining_next.len() {
                    if partitions[remaining_next[j].group as usize] == partition {
                        // scope belongs to this branch of tree
                        let mut next = remaining_next.remove(j);
                        if collapse_empty_scopes && next.scope.variables.len() == 0 {
                            // the universal scope is empty, thus we can merge existential scope afterwards into currents scope
                            assert!(next.next.len() == 1);
                            let existential = next.next.pop().unwrap();
                            for &variable in existential.scope.variables.iter() {
                                node.scope.variables.push(variable);
                                variables.get_mut(variable).scope = node.scope.id;
                            }
                            node.next.extend(existential.next);
                        } else {
                            node.next.push(next);
                        }
                    } else {
                        j += 1;
                    }
                }

                scopes.push(node);
                groups.insert(variable, scopes.len() - 1);

            // TODO: sort clauses
            } else {
                // variable belongs to variable group represented by `partition`
                debug_assert!(partition < variable);
                let new_scope = &mut scopes[groups[&partition]];
                if scope.contains(variable) {
                    new_scope.scope.variables.push(variable);
                }
            }
        }

        info!("detected {} partitions at level {}", scopes.len(), scope.id);

        scopes
    }

    /// Makes a copy of `scope` for every element in `next`.
    /// Renames universal variables if needed
    fn split_universal(
        mut scope: Scope,
        partitions: &Vec<Variable>,
        next: Vec<Box<ScopeNode>>,
        variables: &mut VariableStore<QVariableInfo>,
        clauses: &mut Vec<Clause>,
        occurrences: &mut FxHashMap<Literal, Vec<ClauseId>>,
    ) -> Vec<Box<ScopeNode>> {
        debug_assert!(!next.is_empty());

        if next.len() == 1 {
            // do not need to copy and rename
            let mut node = Box::new(ScopeNode {
                scope: Scope::new(scope.id),
                group: next[0].group,
                next: next,
            });
            node.scope.variables.extend(scope.variables.clone());
            return vec![node];
        }

        scope.variables.sort();

        // more than one successor, have to rename variables
        debug_assert!(next.len() > 1);
        let mut scopes = Vec::new();
        for next_scope in next {
            let mut new_scope = Scope::new(scope.id);

            // mapping from old variables to new copy
            // is modified lazyly below
            let mut renaming = FxHashMap::default();

            // update clauses and occurrence list
            for (i, ref mut clause) in clauses.iter_mut().enumerate() {
                let clause_id = i as ClauseId;
                // check if clause contains variables of inner group
                let needs_renaming = clause.iter().fold(false, |val, &literal| {
                    let info = variables.get(literal.variable());
                    if info.is_universal() {
                        return val;
                    }
                    if info.scope < scope.id {
                        return val;
                    }
                    if partitions[literal.variable() as usize] == next_scope.group {
                        return true;
                    } else {
                        return val;
                    }
                });
                if needs_renaming {
                    for ref mut literal in clause.iter_mut() {
                        if scope.variables.binary_search(&literal.variable()).is_err() {
                            // not a variable of current scope
                            continue;
                        }
                        let var = literal.variable();
                        if !renaming.contains_key(&var) {
                            variables.variables.push(QVariableInfo {
                                scope: scope.id,
                                is_universal: true,
                                copy_of: var,
                            });
                            let new_var = variables.num_variables() as Variable;
                            new_scope.variables.push(new_var);
                            renaming.insert(var, new_var);
                        }
                        let new_var = *renaming.get(&var).unwrap();

                        {
                            let entry = occurrences
                                .entry(**literal)
                                .or_insert_with(|| panic!("inconsistent state"));
                            // remove old occurrence
                            entry
                                .iter()
                                .position(|&other_clause_id| other_clause_id == clause_id)
                                .map(|index| entry.remove(index));
                        }
                        **literal = Literal::new(new_var, literal.signed());
                        let entry = occurrences.entry(**literal).or_insert(Vec::new());
                        entry.push(clause_id);
                    }
                }
            }
            // it can happen that we build universal scopes without variables
            // this gets cleaned-up in the outer existential quantifier

            let mut node = Box::new(ScopeNode {
                scope: new_scope,
                group: next_scope.group,
                next: vec![next_scope],
            });
            scopes.push(node);
        }
        scopes
    }
}
