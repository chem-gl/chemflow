//! Macros utilitarias para reducir boilerplate al definir Artifacts y Steps
//! tipados.
//!
//! Exportadas en la raíz del crate para poder usarlas como:
//!   use chem_core::{typed_artifact, typed_step};

/// Declara un Artifact tipado con derives y ArtifactSpec.
///
/// Formas soportadas:
/// - typed_artifact!(Name { field1: Ty1, field2: Ty2 }); // KIND = GenericJson
///   por defecto
/// - typed_artifact!(Name { field1: Ty1 } kind: $kind_expr );
#[macro_export]
macro_rules! typed_artifact {
    // Con KIND explícito
    ($name:ident { $($fname:ident : $fty:ty),+ $(,)? } kind: $kind:expr) => {
        #[derive(Clone, serde::Serialize, serde::Deserialize)]
        pub struct $name { $(pub $fname: $fty,)+ pub schema_version: u32 }
        impl $crate::model::ArtifactSpec for $name {
            const KIND: $crate::model::ArtifactKind = $kind;
        }
    };
    // KIND por defecto GenericJson
    ($name:ident { $($fname:ident : $fty:ty),+ $(,)? }) => {
        $crate::typed_artifact!($name { $($fname : $fty),+ } kind: $crate::model::ArtifactKind::GenericJson);
    };
}

#[macro_export]
macro_rules! typed_step {
    // ---------------- Source con fields y ctor custom ----------------
    (
        source $name:ident {
            id: $id:expr,
            output: $out:ty,
            params: $params:ty,
            fields { $($fname:ident : $fty:ty),+ $(,)? }
            , ctor (|$($ctor_args:tt)*| $ctor:block)
            , run($self_ident:ident, $p_ident:ident) $body:block
        }
    ) => {
        #[derive(Clone, Debug)]
        pub struct $name { $(pub $fname: $fty),+ }
        impl $name { pub fn new($($ctor_args)*) -> Self { $ctor } }
        impl $crate::step::TypedStep for $name {
            type Params = $params;
            type Input = $out;   // ignorado (Source)
            type Output = $out;
            fn id(&self) -> &'static str { $id }
            fn kind(&self) -> $crate::step::StepKind { $crate::step::StepKind::Source }
            fn params_default(&self) -> Self::Params { <Self::Params as Default>::default() }
            fn run_typed(&self, _input: Option<Self::Input>, $p_ident: Self::Params) -> $crate::step::StepRunResultTyped<Self::Output> {
                let $self_ident = self;
                let out: Self::Output = { $body };
                $crate::step::StepRunResultTyped::Success { outputs: vec![out] }
            }
        }
    };

    // ---------------- Source con fields sin ctor custom ----------------
    (
        source $name:ident {
            id: $id:expr,
            output: $out:ty,
            params: $params:ty,
            fields { $($fname:ident : $fty:ty),+ $(,)? }
            , run($self_ident:ident, $p_ident:ident) $body:block
        }
    ) => {
        #[derive(Clone, Debug)]
        pub struct $name { $(pub $fname: $fty),+ }
        impl $name { pub fn new($($fname : $fty),+) -> Self { Self { $($fname),+ } } }
        impl $crate::step::TypedStep for $name {
            type Params = $params;
            type Input = $out;   // ignorado (Source)
            type Output = $out;
            fn id(&self) -> &'static str { $id }
            fn kind(&self) -> $crate::step::StepKind { $crate::step::StepKind::Source }
            fn params_default(&self) -> Self::Params { <Self::Params as Default>::default() }
            fn run_typed(&self, _input: Option<Self::Input>, $p_ident: Self::Params) -> $crate::step::StepRunResultTyped<Self::Output> {
                let $self_ident = self;
                let out: Self::Output = { $body };
                $crate::step::StepRunResultTyped::Success { outputs: vec![out] }
            }
        }
    };

    // ---------------- Source unit (sin fields) ----------------
    (
        source $name:ident {
            id: $id:expr,
            output: $out:ty,
            params: $params:ty,
            run($self_ident:ident, $p_ident:ident) $body:block
        }
    ) => {
        #[derive(Clone, Debug)]
        pub struct $name;
        impl $name { pub fn new() -> Self { Self } }
        impl $crate::step::TypedStep for $name {
            type Params = $params;
            type Input = $out;   // ignorado (Source)
            type Output = $out;
            fn id(&self) -> &'static str { $id }
            fn kind(&self) -> $crate::step::StepKind { $crate::step::StepKind::Source }
            fn params_default(&self) -> Self::Params { <Self::Params as Default>::default() }
            fn run_typed(&self, _input: Option<Self::Input>, $p_ident: Self::Params) -> $crate::step::StepRunResultTyped<Self::Output> {
                let _step_self = self;
                let out: Self::Output = { $body };
                $crate::step::StepRunResultTyped::Success { outputs: vec![out] }
            }
        }
    };

    // ---------------- Step Transform/Sink con fields y ctor custom ----------------
    (
        step $name:ident {
            id: $id:expr,
            kind: $kind:expr,
            input: $inp:ty,
            output: $out:ty,
            params: $params:ty,
            fields { $($fname:ident : $fty:ty),+ $(,)? }
            , ctor (|$($ctor_args:tt)*| $ctor:block)
            , run($self_ident:ident, $inp_ident:ident, $p_ident:ident) $body:block
        }
    ) => {
        #[derive(Clone, Debug)]
        pub struct $name { $(pub $fname: $fty),+ }
        impl $name { pub fn new($($ctor_args)*) -> Self { $ctor } }
        impl $crate::step::TypedStep for $name {
            type Params = $params;
            type Input = $inp;
            type Output = $out;
            fn id(&self) -> &'static str { $id }
            fn kind(&self) -> $crate::step::StepKind { $kind }
            fn params_default(&self) -> Self::Params { <Self::Params as Default>::default() }
            fn run_typed(&self, input: Option<Self::Input>, $p_ident: Self::Params) -> $crate::step::StepRunResultTyped<Self::Output> {
                let $self_ident = self;
                let $inp_ident: Self::Input = input.expect(concat!("Step ", $id, " requiere input"));
                let out: Self::Output = { $body };
                $crate::step::StepRunResultTyped::Success { outputs: vec![out] }
            }
        }
    };

    // ---------------- Step Transform/Sink con fields sin ctor custom ----------------
    (
        step $name:ident {
            id: $id:expr,
            kind: $kind:expr,
            input: $inp:ty,
            output: $out:ty,
            params: $params:ty,
            fields { $($fname:ident : $fty:ty),+ $(,)? }
            , run($self_ident:ident, $inp_ident:ident, $p_ident:ident) $body:block
        }
    ) => {
        #[derive(Clone, Debug)]
        pub struct $name { $(pub $fname: $fty),+ }
        impl $name { pub fn new($($fname : $fty),+) -> Self { Self { $($fname),+ } } }
        impl $crate::step::TypedStep for $name {
            type Params = $params;
            type Input = $inp;
            type Output = $out;
            fn id(&self) -> &'static str { $id }
            fn kind(&self) -> $crate::step::StepKind { $kind }
            fn params_default(&self) -> Self::Params { <Self::Params as Default>::default() }
            fn run_typed(&self, input: Option<Self::Input>, $p_ident: Self::Params) -> $crate::step::StepRunResultTyped<Self::Output> {
                let $self_ident = self;
                let $inp_ident: Self::Input = input.expect(concat!("Step ", $id, " requiere input"));
                let out: Self::Output = { $body };
                $crate::step::StepRunResultTyped::Success { outputs: vec![out] }
            }
        }
    };

    // ---------------- Step Transform/Sink unit (sin fields) ----------------
    (
        step $name:ident {
            id: $id:expr,
            kind: $kind:expr,
            input: $inp:ty,
            output: $out:ty,
            params: $params:ty,
            run($self_ident:ident, $inp_ident:ident, $p_ident:ident) $body:block
        }
    ) => {
        #[derive(Clone, Debug)]
        pub struct $name;
        impl $name { pub fn new() -> Self { Self } }
        impl $crate::step::TypedStep for $name {
            type Params = $params;
            type Input = $inp;
            type Output = $out;
            fn id(&self) -> &'static str { $id }
            fn kind(&self) -> $crate::step::StepKind { $kind }
            fn params_default(&self) -> Self::Params { <Self::Params as Default>::default() }
            fn run_typed(&self, input: Option<Self::Input>, $p_ident: Self::Params) -> $crate::step::StepRunResultTyped<Self::Output> {
                let _step_self = self;
                let $inp_ident: Self::Input = input.expect(concat!("Step ", $id, " requiere input"));
                let out: Self::Output = { $body };
                $crate::step::StepRunResultTyped::Success { outputs: vec![out] }
            }
        }
    };
}
