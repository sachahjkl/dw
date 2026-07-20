package spec

func dbGrammar(b *builder) *Command {
	connection := func(key string, table bool) []Argument {
		args := make([]Argument, 0, 5)
		if table {
			args = append(args, b.positional(key, "table", "TABLE", String, false, "Table to describe, in table or schema.table format."))
		}
		args = append(args,
			completion(b.option(key, "project", String, "Configured project containing the database connection."), CompleteProject),
			completion(conflict(b.option(key, "database", String, "Connection name declared in databases.json."), "env"), CompleteDatabase),
			completion(conflict(b.option(key, "env", String, "Database environment alias declared in databases.json."), "database"), CompleteEnvironment),
			b.option(key, "json", Bool, "Emit the deterministic JSON result."),
		)
		return args
	}
	query := connection("db.query", false)
	query = append([]Argument{b.option("db.query", "sql", String, "Read-only SQL query to execute.")}, query...)
	query = append(query,
		positive(completion(b.option("db.query", "max-rows", Int, "Maximum number of rows to show."), CompleteRowLimit, "50", "100", "500", "1000")),
		repeat(trailing(b.positional("db.query", "sql_parts", "SQL", Strings, false, "Read-only SQL query to execute."))),
	)
	return b.command("db", "db", "Explore and guard database access.", nil,
		b.command("list", "db.list", "List configured databases without resolving connection secrets.", []Argument{
			b.option("db.list", "root", String, "DevWorkflow root to inspect."), b.option("db.list", "json", Bool, "Emit the deterministic JSON result."),
		}),
		b.command("collect", "db.collect", "Discover database connections in workspace appsettings files.", []Argument{
			b.option("db.collect", "root", String, "DevWorkflow root whose workspaces should be scanned."),
			b.option("db.collect", "save", Bool, "Store eligible values in the system keyring and add credential references to databases.json."),
			b.option("db.collect", "json", Bool, "Emit the deterministic JSON result without secret values."),
		}),
		b.command("guard", "db.guard", "Verify that a SQL query respects read-only mode.", []Argument{
			mandatory(b.option("db.guard", "sql", String, "SQL query to analyze without executing.")),
		}),
		b.command("schema", "db.schema", "List accessible tables and views on a configured database.", connection("db.schema", false)),
		b.command("describe", "db.describe", "Describe the columns of a SQL table.", connection("db.describe", true)),
		b.command("query", "db.query", "Execute a read-only SQL query with guards and a row limit.", query),
	)
}
