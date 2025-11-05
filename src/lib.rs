/*
  Copyright 2025 Will Murnane

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
*/

use pgrx::prelude::*;

::pgrx::pg_module_magic!(name, version);
use serde::Deserialize;
use serde::Serialize;

#[derive(PostgresType, Serialize, Eq, PartialEq, Deserialize, PostgresEq)]
#[serde(transparent)]
#[inoutfuncs]
pub struct AccessExpression(::access::AccessExpression);

impl InOutFuncs for AccessExpression {
    fn input(input: &::std::ffi::CStr) -> Self {
        AccessExpression(::access::expression(input.to_str().unwrap()).unwrap())
    }

    fn output(&self, buffer: &mut ::pgrx::StringInfo) {
        buffer.push_str(&format!("{}", self.0))
    }
}
#[derive(PostgresType, Eq, PartialEq, Serialize, Deserialize, PostgresEq)]
#[serde(transparent)]
#[inoutfuncs]
pub struct AccessTokens(::access::AccessTokens);
impl InOutFuncs for AccessTokens {
    fn input(input: &::std::ffi::CStr) -> Self {
        AccessTokens(::access::tokens(input.to_str().unwrap()).unwrap())
    }

    fn output(&self, buffer: &mut ::pgrx::StringInfo) {
        buffer.push_str(&format!("{}", self.0))
    }
}

#[pg_extern]
pub fn access_evaluate(expression: AccessExpression, tokens: AccessTokens) -> bool {
    ::access::evaluate(&expression.0, &tokens.0)
}
/// This module is required by `cargo pgrx test` invocations.
/// It must be visible at the root of your extension crate.
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    #[must_use]
    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_access_expression() {
        let val = Spi::get_one::<SomeEnum>(r#"SELECT 'a'::AccessExpression"#);
        assert_eq!(val, Ok(Some(AccessExpression::Token("a".to_string()))));
    }
}
