## What is this

[![Linux](https://github.com/willmurnane/access-pgrx/actions/workflows/linux.yml/badge.svg)](https://github.com/willmurnane/access-pgrx/actions/workflows/linux.yml) | [![Macos](https://github.com/willmurnane/access-pgrx/actions/workflows/macos.yml/badge.svg)](https://github.com/willmurnane/access-pgrx/actions/workflows/macos.yml) | [![Windows](https://github.com/willmurnane/access-pgrx/actions/workflows/windows.yml/badge.svg)](https://github.com/willmurnane/access-pgrx/actions/workflows/windows.yml)

This project implements the [accumulo-access specification](https://github.com/apache/accumulo-access/blob/main/SPECIFICATION.md) as [PostgreSQL types](https://www.postgresql.org/docs/current/xtypes.html), using [the pgrx library](https://github.com/pgcentralfoundation/pgrx) to wrap [a Rust implementation of the expression and token types](https://github.com/willmurnane/access-rs).

Artifacts are built for the following platforms:

- Ubuntu Linux on amd64
- Ubuntu Linux on arm64
- Macos on amd64
- Macos on arm64
- Windows on amd64

and PostgreSQL versions from 13 to 18.

The artifacts are currently packaged as `.tar.gz` files for all combinations of PostgreSQL versions and platforms. In addition, for Linux, OCI containers are produced, which should be suitable for use with [CNPG's extensions mechanism](https://cloudnative-pg.io/documentation/current/imagevolume_extensions/).

## What can I do with this

This extension implements the Accumulo access standard, and exposes `AccessExpression` and `AccessTokens` types, along with an `access_evaluate` function. This allows you to build powerful, label-based Row Level Security (RLS) policies to filter data based on user permissions.

### Accumulo access recap

Access tokens are strings which represent some particular aspect of access. The meaning of these tokens is up to you. For example:

- in a system where users from different locations have different levels of access, you might assign tokens `Antigua` and `Barbados` to users from those locations.
- in a system where different departments are not permitted to share certain kinds of data, you might assign one token per department
- in a system with customer support data, you might have different access tokens to represent data available to tier 1 support, tier 2 support, supervisors, etc.

Tokens composed only of the characters `[a-zA-Z0-9_\-\.:/]` (latin alphabet, numbers, or the characters `_-.:/`) may be expressed without quoting. In addition, any valid sequence of non-surrogate UTF-8 code points may be used as a token, as long as the following algorithm is followed to construct a quoted token:

- append a double quote `"` to the quoted token
- for each code point which is to be emitted:
  - if the code point is a double quote `\u0022`, append a backslash and a double-quote `\"`
  - else if the code point is a backslash `\u005c`, append two backslashes `\\`
  - else append the code point
- finally, append a double quote.

Quoted tokens which may be written as unquoted tokens are equivalent to the unquoted form: `'"a"'::accesstokens` is equal to `'a'::accesstokens`.

Note that tokens are case-sensitive, and do not obey [Unicode equivalence](https://en.wikipedia.org/wiki/Unicode_equivalence) rules: `U&'\006E\0303'` and `U&'\00F1'` are different sequences of code points, and thus are unrelated tokens as far as accumulo access is concerned, despite representing the same character: ñ ñ. Apply appropriate normalization (NFC and/or case folding) externally if necessary.

Access expressions are boolean expressions comprised of tokens. When evaluated with respect to a set of tokens, each token in the expression will be treated as `true` if the token is present or `false` if it is absent.

- Individual tokens (quoted or unquoted, as defined above) are valid sub-expressions.
- Sub-expressions may be surrounded with parentheses to disambiguate meaning: `(A)` or even `((((((a))))))` are valid groupings.
- Sub-expressions may be combined with junctions: `&` meaning "and", or `|` meaning "or". Mixing junctions is not permitted: `A&B|C&D` is not permitted, because it could be interpreted as `A&(B|C)&D` or `(A&B)|(C&D)`.

For example, `":)"&Z&("…"|"A")` is a valid access expression.

### Accumulo access as applied

Out of these tokens, two PostgreSQL types are built.

- `AccessExpression`: A boolean expression required to view a piece of data. Conversion to and from string are provided, and the clauses and sub-clauses are put into a canonical order when read. For example, `SELECT '(b&D)|Z|(a|c)'::accessexpression;` returns `Z|a|c|(D&b)`. This has no impact on the meaning of the expression.

- `AccessTokens`: A set of string labels a user possesses. Conversions to and from string are provided, and the tokens will be put into a canonical order when read. For example: `SELECT '":)",A,"…",Z'::accesstokens;` returns `A,Z,":)","…"`, because the value has been parsed into the canonical form, then converted back to a string. This has no impact on how the tokens are evaluated with respect to an expression.

- `access_evaluate(expression, tokens)`: A function that checks if a set of tokens are sufficient for an expression. For example:
  - `access_evaluate('A&(b|c)'::accessexpression, 'A,c'::accesstokens)` returns true, because `A` is sufficient to fulfill the first clause, and `c` is sufficient for the second.
  - `access_evaluate('A&(b|c)'::accessexpression, 'b,c'::accesstokens)` returns false, because although the second clause is fulfilled

## Example Scenario: Users and Auditors

Consider a scenario where a data table contains records visible to different groups:

- Users (e.g., Alice, Bob) work on subsets of the data.
- Auditors (e.g., Frank, Lauren, Cara) also work subsets of data, but the data is sliced in a different way.

We will create a single RLS policy that fetches the user's tokens from a `users` table, and filters the `data` table.

### Enable Extension and Define Schema

First, we enable the extension and create our tables.

```
CREATE EXTENSION IF NOT EXISTS access_pgrx;
-- Table for user permissions
CREATE TABLE users (
    user_id TEXT NOT NULL PRIMARY KEY,
    access_level ACCESSTOKENS NOT NULL
);
-- Grant everyone permission to read from the 'users' table, so their tokens can be retrieved. In this example, knowledge of "who has what level of access" is not considered sensitive.
GRANT SELECT ON users TO PUBLIC;
-- Data table, with row-level access expressions
CREATE TABLE data (
    id SERIAL PRIMARY KEY,
    stuff TEXT NOT NULL,
    restriction ACCESSEXPRESSION NOT NULL
);
```

### Create a Helper Function

Next, we create a function that finds the `AccessTokens` that should be used for the `CURRENT_USER`. This is not strictly necessary in this case, but it's a convenient layer of abstraction.

```
CREATE OR REPLACE FUNCTION get_current_user_tokens()
RETURNS ACCESSTOKENS AS $$
DECLARE
    tokens ACCESSTOKENS;
BEGIN
    -- Check if the current user is in the 'users' table
    SELECT access_level INTO tokens FROM users WHERE user_id = CURRENT_USER;
    IF FOUND THEN
        RETURN tokens;
    END IF;

    -- If user is not found, return empty tokens
    RETURN ''::ACCESSTOKENS;
END;
$$ LANGUAGE plpgsql SECURITY INVOKER;
```

Note that it is strongly recommended to implement an immutable function, and [mark it as such](https://www.postgresql.org/docs/18/sql-createfunction.html). If the function is not marked as `IMMUTABLE`, PostgreSQL will assume that it could change for every row, and re-evaluate it for every row, slowing down the process substantially. The function above is not immutable, because one could potentially call it while modifying the `users` table, but if transactions can be restricted to contain only statements which modify the `users` table or those which make use of the row-level security, it can safely be marked as immutable.

In this example we're relying on PostgreSQL's role system to determine which user is acting, and looking up that role in a table to determine what tokens should be used. There's no restriction that you have to do things the same way in your application: you could instead

- set the credentials for the current session manually in a GUC like `SET myorg.tokens = 'apple,BaNaNa';` from your application on every connection, and then
  `CREATE OR REPLACE FUNCTION get_current_user_tokens() RETURNS ACCESSTOKENS AS $$ SELECT current_setting('myorg.tokens')::accesstokens; $$ LANGUAGE SQL IMMUTABLE;`
- use [PostgREST](https://docs.postgrest.org/en/v14/) and extract a claim from the user's JWT as the tokens to use for the session:
  `CREATE OR REPLACE FUNCTION get_current_user_tokens() RETURNS ACCESSTOKENS AS $$ SELECT current_setting('request.jwt.claims', true)::json->>'claims'::accesstokens $$ LANGUAGE SQL IMMUTABLE;`
- come up with some other arbitrary solution.

### Apply Row Level Security

Now, we enable row level security on the data table and create a single policy that uses our helper function. This policy is simple: it grants access if the user's tokens satisfy the row's restriction expression.

```
ALTER TABLE data ENABLE ROW LEVEL SECURITY;
CREATE POLICY user_and_auditor_access_policy ON data
  FOR ALL
  USING (access_evaluate(restriction, get_current_user_tokens()));
```

### Create Users and Insert Data

Let's create test roles and populate our tables with permissions and restricted data.

```
CREATE ROLE alice LOGIN;
CREATE ROLE bob LOGIN;
CREATE ROLE frank LOGIN;
CREATE ROLE lauren LOGIN;
CREATE ROLE cara LOGIN;

-- Grant them basic permissions to select from the 'data' table
-- RLS will handle the filtering.
GRANT SELECT ON data TO alice, bob, frank, lauren, cara;

-- Populate user and auditor access levels
INSERT INTO users (user_id, access_level) VALUES
    ('alice',  'USER,DEPT_A'),
    ('bob',    'USER,DEPT_A,DEPT_B'),
    ('frank',  'AUDITOR,AUDIT_FINANCE'),
    ('lauren', 'AUDITOR,AUDIT_LEGAL'),
    ('cara',   'AUDITOR,C_SUITE')
    ;

-- Populate the data table with different restrictions
INSERT INTO data (stuff, restriction) VALUES
    ('General User Memo', 'USER|AUDITOR'),
    ('Dept A Balance sheet', '(USER&DEPT_A)|(AUDITOR&(AUDIT_FINANCE|C_SUITE))'),
    ('Dept B Balance sheet', '(USER&DEPT_B)|(AUDITOR&(AUDIT_FINANCE|C_SUITE))'),
    ('Super Secret Strategy', '(AUDITOR&C_SUITE)'),
    ('Cross-Dept Legal Initiative', '(USER&(DEPT_A|DEPT_B))|(AUDITOR&AUDIT_LEGAL)');
```

### Test the Filtering

Now, we can use SET ROLE to impersonate each user and see how the RLS policy filters the data table for them.

```
SET ROLE alice;
SELECT * FROM data;
SET ROLE bob;
SELECT * FROM data;
SET ROLE frank;
SELECT * FROM data;
SET ROLE lauren;
SELECT * FROM data;
SET ROLE cara;
SELECT * FROM data;
RESET ROLE;
```

This produces:

```
-- alice
 id |            stuff            |                   restriction
----+-----------------------------+-------------------------------------------------
  1 | General User Memo           | AUDITOR|USER
  2 | Dept A Balance sheet        | (AUDITOR&(AUDIT_FINANCE|C_SUITE))|(DEPT_A&USER)
  5 | Cross-Dept Legal Initiative | (AUDITOR&AUDIT_LEGAL)|(USER&(DEPT_A|DEPT_B))
(3 rows)

-- bob
 id |            stuff            |                   restriction
----+-----------------------------+-------------------------------------------------
  1 | General User Memo           | AUDITOR|USER
  2 | Dept A Balance sheet        | (AUDITOR&(AUDIT_FINANCE|C_SUITE))|(DEPT_A&USER)
  3 | Dept B Balance sheet        | (AUDITOR&(AUDIT_FINANCE|C_SUITE))|(DEPT_B&USER)
  5 | Cross-Dept Legal Initiative | (AUDITOR&AUDIT_LEGAL)|(USER&(DEPT_A|DEPT_B))

-- frank
 id |        stuff         |                   restriction
----+----------------------+-------------------------------------------------
  1 | General User Memo    | AUDITOR|USER
  2 | Dept A Balance sheet | (AUDITOR&(AUDIT_FINANCE|C_SUITE))|(DEPT_A&USER)
  3 | Dept B Balance sheet | (AUDITOR&(AUDIT_FINANCE|C_SUITE))|(DEPT_B&USER)

-- lauren
 id |            stuff            |                 restriction
----+-----------------------------+----------------------------------------------
  1 | General User Memo           | AUDITOR|USER
  5 | Cross-Dept Legal Initiative | (AUDITOR&AUDIT_LEGAL)|(USER&(DEPT_A|DEPT_B))
(2 rows)
-- cara
 id |         stuff         |                   restriction
----+-----------------------+-------------------------------------------------
  1 | General User Memo     | AUDITOR|USER
  2 | Dept A Balance sheet  | (AUDITOR&(AUDIT_FINANCE|C_SUITE))|(DEPT_A&USER)
  3 | Dept B Balance sheet  | (AUDITOR&(AUDIT_FINANCE|C_SUITE))|(DEPT_B&USER)
  4 | Super Secret Strategy | AUDITOR&C_SUITE
```

### End notes on this example

- In this example the user and auditor roles are assigned the `USER` and `AUDITOR` tokens respectively. This is not necessary; for example, the decision could be made that "the `AUDIT_FINANCE` token implies this role is an auditor, so we don't need to assign an `AUDITOR` token explicitly" and similarly "`DEPT_A` implies `USER` so we'll just leave it out". Then the `restriction` column could be trimmed down: for example, `(AUDITOR&(AUDIT_FINANCE|C_SUITE))|(DEPT_B&USER)` could turn into `AUDIT_FINANCE|C_SUITE|DEPT_B`. This would be faster to evaluate and smaller to store, but potentially more error-prone to manage.
- In this example, it's possible to assign a particular role both user and auditor tokens simultaneously. In a real application there may be restrictions on this sort of thing; the `get_current_user_tokens` function or equivalent could enforce this restriction, by `RAISE EXCEPTION 'Cannot be simultaneously user and auditor`' for example.
