use crate::Result;
use rune::{
    prepare,
    runtime::GuardedArgs,
    termcolor::{ColorChoice, StandardStream},
    Diagnostics, FromValue, Module, Source, Sources, Vm,
};
use std::sync::Arc;

#[allow(dead_code)]
pub async fn run<T: FromValue, A: GuardedArgs>(
    modules: Vec<Module>,
    code: &str,
    args: A,
) -> Result<T> {
    let mut context = rune::Context::with_default_modules()?;

    for module in modules {
        context.install(module)?;
    }

    let mut sources = Sources::new();
    sources.insert(Source::memory(format!(
        "
        pub async fn main(value) {{
            {code}
        }}
        ",
        code = code
    ))?)?;

    let mut diagnostics = Diagnostics::new();
    let result = prepare(&mut sources)
        .with_context(&context)
        .with_diagnostics(&mut diagnostics)
        .build();

    if !diagnostics.is_empty() {
        let mut writer = StandardStream::stderr(ColorChoice::Always);
        diagnostics.emit(&mut writer, &sources)?;
    }

    let mut vm = Vm::new(Arc::new(context.runtime()?), Arc::new(result?));

    let value = vm.async_call(["main"], args).await?;

    rune::from_value::<Result<T>>(value)?
}
