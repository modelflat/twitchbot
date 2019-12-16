use rlua::{HookTriggers, Error};

/// Sandbox supplied user code on lua side
fn sandbox(untrusted_code: &str) -> String {
    format!(r#"
local env = {{}}

local function run(untrusted_code)
  local untrusted_function, message = load(untrusted_code, nil, 't', env)
  if not untrusted_function then
    return "C: "..message
  end
  local status, result = pcall(untrusted_function)
  if result == nil then
    return "nil"
  else
    if status then
      return result
    else
      return "R: "..result
    end
  end
end

return run([[
    {code}
]])
"#, code = untrusted_code)
}

/// Runs lua code in a sandbox.
pub fn run_untrusted_lua_code(source_code: String) -> Result<String, String> {
    let source_code = sandbox(&source_code);

    let mut instructions = 1 << 10;

    let vm = rlua::Lua::new();

    // 640 kilobytes ought to be enough for anyone
    vm.set_memory_limit(Some(640 * (1 << 10)));

    vm.set_hook(HookTriggers {
        every_nth_instruction: Some(1),
        ..Default::default()
    }, move |_lua, _debug| {
        instructions -= 1;
        if instructions < 0 {
            Err(Error::RuntimeError("execution timeout!".to_string()))
        } else {
            Ok(())
        }
    });

    vm.context(|context| {
        match context.load(&source_code).into_function() {
            Ok(compiled) => {
                match compiled.call::<_, String>(0) {
                    Ok(result) =>
                        Ok(format!("{:?}", result)),
                    Err(err) =>
                        Err(format!("R: {:?}", err)),
                }
            },
            Err(err) => Err(format!("C: {:?}", err)),
        }
    })
}
