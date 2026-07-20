package spec

func providerOption(b *builder, key string) Argument {
	return completion(b.option(key, "provider", String, "Provider to use; defaults to the configured project provider."), CompleteProvider)
}

func providerPositional(b *builder, key, help string) Argument {
	return completion(b.positional(key, "provider", "PROVIDER", String, true, help), CompleteProvider)
}

func providerGrammar(b *builder) *Command {
	auth := b.command("auth", "provider.auth", "Manage provider authentication.", nil,
		b.command("login", "provider.auth.login", "Connect a provider account.", []Argument{
			providerPositional(b, "provider.auth.login", "Provider to authenticate."),
			b.option("provider.auth.login", "root", String, "DevWorkflow root to use for authentication configuration."),
		}),
		b.command("status", "provider.auth.status", "Show provider connection status.", []Argument{
			providerPositional(b, "provider.auth.status", "Provider whose connection status should be shown."),
			b.option("provider.auth.status", "root", String, "DevWorkflow root to use for authentication configuration."),
		}),
		b.command("logout", "provider.auth.logout", "Remove a local provider session.", []Argument{
			providerPositional(b, "provider.auth.logout", "Provider whose local session should be removed."),
			b.option("provider.auth.logout", "root", String, "DevWorkflow root to use for authentication configuration."),
		}),
	)
	return b.command("provider", "provider", "Inspect providers and manage provider authentication.", nil,
		b.command("list", "provider.list", "List registered work and data providers.", []Argument{
			b.option("provider.list", "json", Bool, "Emit the deterministic JSON response."),
		}),
		b.command("show", "provider.show", "Show provider registration and capability details.", []Argument{
			providerPositional(b, "provider.show", "Provider to inspect."),
			b.option("provider.show", "json", Bool, "Emit the deterministic JSON response."),
		}),
		b.command("capabilities", "provider.capabilities", "List capabilities exposed by a provider.", []Argument{
			providerPositional(b, "provider.capabilities", "Provider whose capabilities should be listed."),
			b.option("provider.capabilities", "json", Bool, "Emit the deterministic JSON response."),
		}),
		auth,
	)
}
