package spec

func webGrammar(builder *builder) *Command {
	rootOption := func(key string) Argument {
		return builder.option(key, "root", String, "DevWorkflow root to serve.")
	}
	portOption := func(key string) Argument {
		argument := builder.option(key, "port", Int, "Loopback port from 0 to 65535.")
		argument.Validate = ValidatePort
		return argument
	}
	serve := builder.command("serve", "web.serve", "Run the local web server in the foreground.", nil)
	serve.Hidden = true
	return builder.command("web", "web", "Manage the local DevWorkflow web service.", nil,
		builder.command("start", "web.start", "Start the local web service.", []Argument{
			rootOption("web.start"),
			portOption("web.start"),
			builder.option("web.start", "no-open", Bool, "Do not open the browser."),
		}),
		builder.command("stop", "web.stop", "Stop the local web service.", nil),
		builder.command("status", "web.status", "Show the local web service status.", []Argument{
			builder.option("web.status", "json", Bool, "Emit the deterministic JSON report."),
		}),
		builder.command("open", "web.open", "Open the local web service in a browser.", nil),
		builder.command("register", "web.register", "Register the local web service for user login.", []Argument{
			rootOption("web.register"),
			portOption("web.register"),
		}),
		builder.command("unregister", "web.unregister", "Remove the local web service registration.", nil),
		serve,
	)
}
