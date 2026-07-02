namespace Dw.Cli.Contracts;

internal static class WorkflowContracts
{
    internal static class Ado
    {
        public const string WorkItemTypeUserStory = "User Story";
        public const string WorkItemTypeAnomaly = "Anomalie";
        public const string WorkItemTypeBug = "Bug";
        public const string WorkItemTypeTask = "Task";
        public const string WorkItemTypeTaskFr = "Tache";
        public const string WorkItemTypeActivity = "Activité";
        public const string WorkItemTypeActivityAscii = "Activite";

        public const string StateValidated = "Validé";
        public const string StateValidatedAscii = "Valide";
        public const string StateClosed = "Clôturé";
        public const string StateClosedAscii = "Cloturé";
        public const string StateAbandoned = "Abandonné";
        public const string StateAbandonedAscii = "Abandonne";
        public const string StateInProgress = "En réalisation";
        public const string StateDevelopment = "En développement";
        public const string StatePrPending = "PR en attente";

        public const string RelationHierarchyReverse = "System.LinkTypes.Hierarchy-Reverse";
        public const string RelationHierarchyForward = "System.LinkTypes.Hierarchy-Forward";
        public const string RelationDependencyReverse = "System.LinkTypes.Dependency-Reverse";
        public const string RelationDependencyForward = "System.LinkTypes.Dependency-Forward";
        public const string RelationAttachedFile = "AttachedFile";

        public const string NormalizedWorkItemTypeUserStory = "user story";
        public const string NormalizedWorkItemTypeAnomaly = "anomalie";
        public const string NormalizedWorkItemTypeBug = "bug";
        public const string NormalizedWorkItemTypeTask = "task";
        public const string NormalizedWorkItemTypeTaskFr = "tache";
        public const string NormalizedWorkItemTypeActivity = "activité";
        public const string NormalizedWorkItemTypeActivityAscii = "activite";

        public const string NormalizedStateValidated = "validé";
        public const string NormalizedStateValidatedAscii = "valide";
        public const string NormalizedStateClosed = "clôturé";
        public const string NormalizedStateClosedAscii = "cloturé";
        public const string NormalizedStateAbandoned = "abandonné";
        public const string NormalizedStateAbandonedAscii = "abandonne";
    }

    internal static class Workspace
    {
        public const string ManifestFileName = "task.json";
        public const string PlanFileName = "plan.md";
        public const string AgentsFileName = "AGENTS.md";
        public const string HandoffPrefix = "handoff-";
        public const string MarkdownExtension = ".md";
        public const string AttachmentDirectoryPrefix = "attachments/ado/";
    }

    internal static class Repositories
    {
        public const string Front = "front";
        public const string Back = "back";
        public const string Db = "db";

        public const string FrontPrefix = "FRONT";
        public const string BackPrefix = "BACK";
    }

    internal static class Schemas
    {
        public const string AdoAiContext = "dw.ado.ai-context.v1";
        public const string TaskPreflight = "dw.task.preflight.v1";
        public const string TaskHandoffValidation = "dw.task.handoff-validation.v1";
    }

    internal static class Handoff
    {
        public const string YamlFenceStart = "```yaml";
        public const string FenceEnd = "```";

        public const string Status = "status";
        public const string Repository = "repository";

        public const string SectionSummary = "summary";
        public const string SectionVerification = "verification";
        public const string SectionArtifacts = "artifacts";

        public const string Done = "done";
        public const string Decisions = "decisions";
        public const string Risks = "risks";
        public const string Blockers = "blockers";
        public const string FollowUp = "follow_up";
        public const string Commands = "commands";
        public const string ManualChecks = "manual_checks";
        public const string Files = "files";
        public const string Screenshots = "screenshots";
        public const string Attachments = "attachments";

        public const string StatusTodo = "todo";
        public const string StatusInProgress = "in_progress";
        public const string StatusDone = "done";
        public const string StatusBlocked = "blocked";
        public const string StatusValid = "valid";
        public const string StatusMissing = "missing";
        public const string StatusInvalid = "invalid";
    }

    internal static class Preflight
    {
        public const string SeverityBlocking = "blocking";
        public const string SeverityWarning = "warning";

        public const string CodePredecessorsActive = "ado.predecessors.active";
        public const string CodeChildrenActive = "ado.children.active";
        public const string CodeContextStale = "workspace.ado-context.stale";
        public const string CodeAttachmentsPresent = "ado.attachments.present";
    }
}
