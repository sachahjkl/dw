package spec

func dataGrammar(b *builder) *Command {
	connection := func(key string, resource bool) []Argument {
		args := make([]Argument, 0, 6)
		if resource {
			args = append(args, b.positional(key, "object", "RESOURCE", String, false, "Data resource to describe, optionally qualified by its container."))
		}
		args = append(args,
			completion(b.option(key, "project", String, "Configured project containing the data source."), CompleteProject),
			providerOption(b, key),
			completion(conflict(b.option(key, "source", String, "Configured data source name."), "env"), CompleteDataSource),
			completion(conflict(b.option(key, "env", String, "Data source environment alias."), "source"), CompleteDataEnvironment),
			b.option(key, "json", Bool, "Emit the deterministic JSON result."),
		)
		return args
	}
	query := connection("data.query", false)
	query = append([]Argument{b.option("data.query", "query", String, "Read-only query to execute.")}, query...)
	query = append(query,
		positive(completion(b.option("data.query", "max-rows", Int, "Maximum number of rows to show."), CompleteRowLimit, "50", "100", "500", "1000")),
		repeat(trailing(b.positional("data.query", "query_parts", "QUERY", Strings, false, "Read-only query to execute."))),
	)
	source := b.command("source", "data.source", "Inspect and collect configured data sources.", nil,
		b.command("list", "data.source.list", "List configured data sources without resolving connection secrets.", []Argument{
			b.option("data.source.list", "root", String, "DevWorkflow root to inspect."),
			providerOption(b, "data.source.list"),
			b.option("data.source.list", "json", Bool, "Emit the deterministic JSON result."),
		}),
		b.command("collect", "data.source.collect", "Discover data source connections in workspace configuration files.", []Argument{
			b.option("data.source.collect", "root", String, "DevWorkflow root whose workspaces should be scanned."),
			providerOption(b, "data.source.collect"),
			b.option("data.source.collect", "save", Bool, "Store eligible values in the system keyring and add credential references to data source configuration."),
			b.option("data.source.collect", "json", Bool, "Emit the deterministic JSON result without secret values."),
		}),
	)
	return b.command("data", "data", "Explore and guard provider-neutral data sources.", nil,
		source,
		b.command("guard", "data.guard", "Verify that a query respects read-only mode.", []Argument{
			mandatory(b.option("data.guard", "query", String, "Query to analyze without executing.")),
			providerOption(b, "data.guard"),
		}),
		b.command("catalog", "data.catalog", "List accessible resources in a configured data source.", connection("data.catalog", false)),
		b.command("describe", "data.describe", "Describe the fields of a data resource.", connection("data.describe", true)),
		b.command("query", "data.query", "Execute a read-only query with guards and a row limit.", query),
	)
}
