use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum TaskCommand {
    #[command(about = "Liste les workspaces task detectes sous le root.")]
    Status {
        #[arg(long, help = "Root DevWorkflow a scanner.")]
        root: Option<String>,
    },
    #[command(about = "Liste les workspaces task avec filtres projet/work item.")]
    List {
        #[arg(long, help = "Root DevWorkflow a scanner.")]
        root: Option<String>,
        #[arg(long, help = "Projet configure a filtrer.")]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item a filtrer.")]
        work_item: Option<String>,
        #[arg(long, help = "Emettre la liste JSON deterministe.")]
        json: bool,
    },
    #[command(about = "Affiche le workspace task courant depuis le repertoire actuel.")]
    Current {
        #[arg(long, help = "Emettre le workspace courant en JSON deterministe.")]
        json: bool,
    },
    #[command(about = "Ouvre ou reprend un workspace task avec l'agent configure.")]
    Open {
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Chemin du workspace a ouvrir directement.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow a utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Projet configure utilise pour resoudre le workspace."
        )]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilise pour resoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus recent correspondant."
        )]
        r#continue: bool,
        #[arg(long, help = "Repository a ouvrir dans le workspace.")]
        repo: Option<String>,
        #[arg(
            long,
            help = "Agent a lancer: opencode, cursor, claude, codex ou copilot."
        )]
        agent: Option<String>,
        #[arg(long, help = "Emettre la resolution JSON au lieu de lancer l'agent.")]
        json: bool,
        #[arg(help = "Alias positionnel du work item pour resoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Prepare ou cree un workspace task depuis des work items ADO.")]
    Start {
        #[arg(help = "Identifiant du work item ADO parent ou enfant a demarrer.")]
        work_item_id: Option<String>,
        #[arg(long, help = "Root DevWorkflow a utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Projet configure a utiliser.")]
        project: Option<String>,
        #[arg(
            long = "task",
            help = "Identifiant de la tache enfant a ajouter au workspace."
        )]
        task: Option<String>,
        #[arg(
            long = "type",
            help = "Type de branche/workspace: feature, bugfix, hotfix ou chore."
        )]
        type_name: Option<String>,
        #[arg(
            long = "only",
            help = "Repository a inclure; repetable via selection interactive si omis."
        )]
        only: Option<String>,
        #[arg(long, help = "Slug explicite pour le nom de branche et workspace.")]
        slug: Option<String>,
        #[arg(
            long = "skip-ado",
            help = "Ne pas interroger Azure DevOps; utiliser les valeurs locales fournies."
        )]
        skip_ado: bool,
        #[arg(long, help = "Emet le plan ou resultat en JSON deterministe.")]
        json: bool,
        #[arg(
            long,
            help = "Cree vraiment le workspace; sans ce flag, affiche le plan."
        )]
        execute: bool,
    },
    #[command(about = "Valide les bloqueurs et avertissements avant implementation.")]
    Preflight {
        #[arg(long, help = "Chemin du workspace a auditer.")]
        workspace: String,
        #[arg(
            long = "ai-context-file",
            help = "Fichier de contexte IA additionnel a verifier; option repetable."
        )]
        ai_context_file: Vec<String>,
        #[arg(long, help = "Emettre le rapport preflight JSON deterministe.")]
        json: bool,
    },
    #[command(about = "Synchronise task.json avec les work items Azure DevOps.")]
    Sync {
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Chemin du workspace a synchroniser.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow a utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Projet configure utilise pour resoudre le workspace."
        )]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilise pour resoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus recent correspondant."
        )]
        r#continue: bool,
        #[arg(long, help = "Emettre le resultat JSON deterministe.")]
        json: bool,
        #[arg(help = "Alias positionnel du work item pour resoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Renomme un workspace task et sa branche selon un nouveau slug.")]
    Rename {
        #[arg(help = "Nouveau slug pour le workspace et la branche.")]
        slug: String,
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Chemin du workspace a renommer.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow a utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Projet configure utilise pour resoudre le workspace."
        )]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilise pour resoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus recent correspondant."
        )]
        r#continue: bool,
        #[arg(long, help = "Emettre le plan/resultat JSON deterministe.")]
        json: bool,
        #[arg(
            long,
            help = "Appliquer vraiment le rename; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(help = "Alias positionnel du work item pour resoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Met les repositories du workspace a jour depuis leur branche cible.")]
    RepoLatest {
        #[arg(
            long,
            conflicts_with = "continue",
            help = "Chemin du workspace a synchroniser."
        )]
        workspace: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus recent correspondant."
        )]
        r#continue: bool,
        #[arg(
            long = "only",
            help = "Limiter la synchronisation a un repository du workspace."
        )]
        only: Option<String>,
        #[arg(long, help = "Root DevWorkflow a utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Emettre le plan/resultat JSON deterministe.")]
        json: bool,
    },
    #[command(
        about = "Prepare ou cree un commit intermediaire pour les repositories du workspace."
    )]
    Commit {
        #[arg(
            long,
            conflicts_with = "continue",
            help = "Chemin du workspace a committer."
        )]
        workspace: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus recent correspondant."
        )]
        r#continue: bool,
        #[arg(long, help = "Root DevWorkflow a utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            help = "Creer vraiment les commits; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(
            long,
            help = "Message de commit explicite; sinon genere depuis le manifeste task."
        )]
        message: Option<String>,
        #[arg(long, help = "Emettre le rapport JSON deterministe.")]
        json: bool,
    },
    #[command(about = "Ajoute des work items au workspace task courant.")]
    AddWorkItem {
        #[arg(help = "IDs de work items a ajouter, separes par virgules.")]
        work_item_ids: String,
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Chemin du workspace a modifier.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow a utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Projet configure utilise pour resoudre le workspace."
        )]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilise pour resoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus recent correspondant."
        )]
        r#continue: bool,
        #[arg(
            long = "skip-ado",
            help = "Ne pas interroger Azure DevOps; utiliser les valeurs fournies localement."
        )]
        skip_ado: bool,
        #[arg(
            long = "type",
            help = "Type local a utiliser si ADO est ignore ou incomplet."
        )]
        type_name: Option<String>,
        #[arg(long, help = "Titre local a utiliser si ADO est ignore ou incomplet.")]
        title: Option<String>,
        #[arg(long, help = "Etat local a utiliser si ADO est ignore ou incomplet.")]
        state: Option<String>,
        #[arg(
            long,
            help = "Appliquer vraiment la modification; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(long, help = "Emettre le plan/resultat JSON deterministe.")]
        json: bool,
        #[arg(help = "Alias positionnel du work item pour resoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Retire des work items du workspace task courant.")]
    RemoveWorkItem {
        #[arg(help = "IDs de work items a retirer, separes par virgules.")]
        work_item_ids: String,
        #[arg(long, help = "Chemin du workspace a modifier.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow a utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Projet configure utilise pour resoudre le workspace.")]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilise pour resoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            help = "Reprendre le workspace task le plus recent correspondant."
        )]
        r#continue: bool,
        #[arg(
            long,
            help = "Appliquer vraiment la modification; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(long, help = "Emettre le plan/resultat JSON deterministe.")]
        json: bool,
        #[arg(help = "Alias positionnel du work item pour resoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Ajoute un repository au workspace task.")]
    AddRepo {
        #[arg(help = "Repository configure a ajouter au workspace.")]
        repo: String,
        #[arg(long, help = "Chemin du workspace a modifier.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow a utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            help = "Creer le worktree et modifier task.json; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(long, help = "Emettre le plan/resultat JSON deterministe.")]
        json: bool,
    },
    #[command(about = "Cree une tache enfant ADO et l'ajoute au handoff repository.")]
    CreateChildTask {
        #[arg(
            long,
            help = "Repository du workspace qui portera le handoff de la tache."
        )]
        repo: String,
        #[arg(long, help = "Titre de la tache enfant ADO a creer.")]
        title: String,
        #[arg(long, help = "Chemin du workspace a modifier.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow a utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Projet configure utilise pour resoudre le workspace.")]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilise pour resoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            help = "Reprendre le workspace task le plus recent correspondant."
        )]
        r#continue: bool,
        #[arg(long, help = "Emettre le resultat JSON deterministe.")]
        json: bool,
        #[arg(help = "Alias positionnel du work item pour resoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Verifie, commit, push et ouvre une PR pour terminer le workspace.")]
    Finish {
        #[arg(
            long,
            conflicts_with = "continue",
            help = "Chemin du workspace a terminer."
        )]
        workspace: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus recent correspondant."
        )]
        r#continue: bool,
        #[arg(long, help = "Root DevWorkflow a utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            help = "Executer les commits, pushs, PR et mises a jour ADO; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(
            long,
            help = "Message de commit explicite; sinon genere depuis le manifeste task."
        )]
        message: Option<String>,
        #[arg(
            long = "create-pr",
            help = "Creer ou verifier les pull requests Azure DevOps apres push."
        )]
        create_pr: bool,
        #[arg(
            long,
            requires = "create_pr",
            help = "Creer les PR en etat ready au lieu de draft."
        )]
        ready: bool,
        #[arg(
            long = "skip-verify",
            help = "Ignorer les commandes de verification configurees avant PR."
        )]
        skip_verify: bool,
        #[arg(
            long = "skip-ado",
            help = "Ne pas appeler Azure DevOps; incompatible avec --create-pr."
        )]
        skip_ado: bool,
        #[arg(long, help = "Emettre le rapport JSON deterministe.")]
        json: bool,
    },
    #[command(about = "Valide les fichiers handoff avant sous-agents ou finition.")]
    HandoffValidate {
        #[arg(
            long,
            help = "Chemin du workspace dont les handoffs doivent etre valides."
        )]
        workspace: String,
        #[arg(long, help = "Emettre le rapport JSON deterministe.")]
        json: bool,
    },
    #[command(about = "Supprime les worktrees et nettoie un workspace task.")]
    Teardown {
        #[arg(long, help = "Chemin du workspace a supprimer.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow a utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Projet configure utilise pour resoudre le workspace.")]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilise pour resoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            help = "Reprendre le workspace task le plus recent correspondant."
        )]
        r#continue: bool,
        #[arg(
            long,
            help = "Supprimer vraiment les worktrees et le workspace; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(long, help = "Confirmer la suppression destructive avec --execute.")]
        yes: bool,
        #[arg(long, help = "Emettre le plan/resultat JSON deterministe.")]
        json: bool,
        #[arg(help = "Alias positionnel du work item pour resoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Nettoie les workspaces dont les work items sont termines.")]
    Prune {
        #[arg(long, help = "Root DevWorkflow a scanner.")]
        root: Option<String>,
        #[arg(long, help = "Projet configure a filtrer.")]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item a filtrer.")]
        work_item: Option<String>,
        #[arg(
            long,
            help = "Supprimer vraiment les workspaces eligibles; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(long, help = "Confirmer la suppression destructive avec --execute.")]
        yes: bool,
        #[arg(
            long = "no-sync",
            help = "Ne pas synchroniser les etats ADO avant de determiner l'eligibilite."
        )]
        no_sync: bool,
        #[arg(long, help = "Emettre le plan/resultat JSON deterministe.")]
        json: bool,
    },
}
