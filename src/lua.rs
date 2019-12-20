use rlua::{Error, HookTriggers, Context, Value};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Arc;
use std::convert::TryFrom;

#[derive(Clone)]
pub enum ExecutionStatus {
    Success = 0,
    CompilationError = 1,
    RuntimeError = 2,
}

impl TryFrom<i32> for ExecutionStatus {

    type Error = Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ExecutionStatus::Success),
            1 => Ok(ExecutionStatus::CompilationError),
            2 => Ok(ExecutionStatus::RuntimeError),
            _ => Err(Error::FromLuaConversionError {
                from: "i32", to: "ExecutionStatus", message: None
            })
        }
    }
}

impl rlua::FromLua<'_> for ExecutionStatus {
    fn from_lua(lua_value: Value<'_>, _: Context<'_>) -> Result<Self, Error> {
        match lua_value {
            Value::Integer(val) => Ok(ExecutionStatus::try_from(val as i32)?),
            _ => Err(Error::FromLuaConversionError {
                from: "<lua value>", to: "ExecutionStatus", message: None
            })
        }
    }
}

/// Sandbox supplied user code on lua side
fn sandbox(untrusted_code: &str) -> String {
    format!(
        r#"
local env = {{}}

local function run(untrusted_code)
  local untrusted_function, message = load(untrusted_code, nil, 't', env)
  if not untrusted_function then
    return {compilation_failed}, message
  end
  local status, result = pcall(untrusted_function)
  if result == nil then
    return "nil"
  else
    if status then
      return {success}, result
    else
      return {runtime_error}, result
    end
  end
end

return run([[
    {code}
]])
"#,
        code = untrusted_code,
        compilation_failed = ExecutionStatus::CompilationError as i32,
        runtime_error = ExecutionStatus::RuntimeError as i32,
        success = ExecutionStatus::Success as i32,
    )
}

pub struct SuccessfulExecution {
    pub instructions_left: isize,
    pub result: String,
}

fn strip_location(s: &str) -> &str {
    if  s.starts_with("[string") {
        if let Some(loc1) = s.find("]:") {
            if let Some(loc2) = &s[loc1..].find(' ') {
                return &s[loc1 + *loc2..].trim()
            }
        }
    }
    s
}

/// Runs lua code in a sandbox.
pub fn run_untrusted_lua_code(source_code: String, instruction_limit: i32, memory_limit: usize)
    -> Result<SuccessfulExecution, String>
{
    let source_code = sandbox(&source_code);

    let vm = rlua::Lua::new();

    let instructions = Arc::new(AtomicIsize::new(instruction_limit as isize));
    let ref_instructions = instructions.clone();
    let timeout_raised = Arc::new(AtomicBool::new(false));
    let ref_timeout_raised = timeout_raised.clone();

    vm.set_memory_limit(Some(memory_limit));

    vm.set_hook(
        HookTriggers {
            every_nth_instruction: Some(1),
            ..Default::default()
        },
        move |_lua, _debug| {
            if instructions.fetch_sub(1, Ordering::SeqCst) < 1 {
                timeout_raised.store(true, Ordering::SeqCst);
                Err(Error::RuntimeError("execution timeout!".to_string()))
            } else {
                Ok(())
            }
        },
    );

    vm.context(|context| match context.load(&source_code).into_function() {
        Ok(compiled) => match compiled.call::<_, (ExecutionStatus, String)>(0) {
            Ok((ExecutionStatus::Success, result)) => {
                Ok(SuccessfulExecution {
                    instructions_left: ref_instructions.load(Ordering::SeqCst),
                    result: format!("{}", result)
                })
            },
            Ok((ExecutionStatus::CompilationError, s)) | Ok((ExecutionStatus::RuntimeError, s)) => {
                Err(format!("ERROR: {}", strip_location(&s)))
            },
            Err(err) => {
                if ref_timeout_raised.load(Ordering::SeqCst) {
                    Err("ERROR: instruction limit reached".to_string())
                } else {
                    Err(format!("ERROR: {:?}", err))
                }
            },
        },
        Err(err) => Err(format!("ERROR: {:?}", err)),
    })
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_can_execute_normally() {
        let result = run_untrusted_lua_code(r#"
        local x = "123"
        for i=1,2 do
            x = x..x
        end
        return x
        "#.to_string(), 100000, 32 * (1 << 10));

        let expected = "123123123123".to_string();

        match result {
            Ok(SuccessfulExecution {result, ..}) => assert_eq!(result, expected, "wrong result"),
            Err(e) => assert!(false, "execution error: {}", e),
        };
    }

    #[test]
    fn test_instruction_limit_is_respected() {
        let result = run_untrusted_lua_code(r#"
        local x = 123
        for i=1,100 do
            x = x + 1
        end
        return x
        "#.to_string(), 100, 32 * (1 << 10));

        let expected_error = "ERROR: instruction limit reached".to_string();

        match result {
            Ok(SuccessfulExecution { result, .. }) =>
                assert!(false, "should abort with error, returned '{}' instead", result),
            Err(e) => assert_eq!(e, expected_error, "wrong error"),
        };
    }

    #[test]
    fn test_memory_limit_is_respected() {
        let result = run_untrusted_lua_code(r#"
        local x = "1234"
        for i=1,100 do
            x = x .. x
        end
        return x
        "#.to_string(), 1000000, 32 * (1 << 10));

        let expected_error = "ERROR: not enough memory".to_string();

        match result {
            Ok(SuccessfulExecution { result, .. }) =>
                assert!(false, "should abort with error, returned '{}' instead", result),
            Err(e) => assert_eq!(e, expected_error, "wrong error"),
        };
    }

    #[test]
    fn test_compilation_error() {
        let result = run_untrusted_lua_code(r#"
        local x = "1234"
        for end
        return x
        "#.to_string(), 1000, 32 * (1 << 10));

        match result {
            Ok(SuccessfulExecution { result, .. }) =>
                assert!(false, "should abort with error, returned '{}' instead", result),
            Err(_) => assert!(true, "should abort with error"),
        };
    }

}
