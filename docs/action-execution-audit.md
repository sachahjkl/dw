# Audit Du Systeme D'Execution D'Actions

## Verdict

Non, nous n'avons pas actuellement un executor generique d'actions asynchrones, identifiees, pausables et annulables.

Nous avons trois mecanismes separes :

1. Un dispatcher synchrone d'actions typees.
2. Une file asynchrone locale, specifique a la TUI.
3. Des operations metier qui acceptent un `context.Context`.

La separation metier-presentation est bonne. Elle facilite un futur usage web.

En revanche, le systeme actuel ne fournit pas les garanties necessaires a une execution distante et durable.

## Architecture Actuelle

### Action typee

Une action possede un `ActionID` stable :

- `internal/contract/types.go:23`
- `internal/action/types.go:13-19`

La requete et le resultat exposent ce meme identifiant.

Exemple conceptuel :

```text
workspace.start
workspace.finish
work.item.show
upgrade.run
```

Cet identifiant decrit le type d'action. Il n'identifie pas une execution particuliere.

### Dispatcher

Le dispatcher associe un `ActionID` a un handler :

- `internal/action/dispatcher.go:57-65`
- `internal/action/dispatcher.go:89-110`

Il verifie :

- que la requete existe ;
- que le handler existe ;
- que le resultat existe ;
- que le resultat correspond a l'action demandee.

Le dispatcher est synchrone :

```go
result, err := handler.Execute(ctx, request, runtime)
```

Il ne gere pas :

- une file ;
- un identifiant d'execution ;
- un statut ;
- une annulation ciblee ;
- une pause ;
- une persistance ;
- une reprise apres redemarrage.

Son verrou protege seulement le registre des handlers. Il ne protege pas les operations metier concurrentes.

### Runtime interactif

Le runtime fournit deux ports :

- `EventSink` pour la progression ;
- `InputPort` pour demander une saisie.

Voir `internal/action/runtime.go:8-35`.

C'est une bonne abstraction. La CLI, la TUI et un serveur web peuvent fournir des adaptateurs differents.

Cependant, `InputPort.Request` est un appel bloquant. Pour le web, il faudrait conserver la requete en attente apres une deconnexion ou un redemarrage.

### CLI

La CLI construit la requete et appelle directement le dispatcher :

- `internal/cli/controller/controller.go:123-157`

L'execution est synchrone du point de vue de la commande CLI.

Le programme utilise `context.Background()` :

- `cmd/dw/main.go:11-16`

Il n'existe pas de `signal.NotifyContext`. Un `Ctrl+C` termine donc le processus sans annulation applicative controlee.

### TUI

La TUI ajoute sa propre couche asynchrone :

- goroutine d'execution : `internal/tui/run.go:283-325` ;
- file FIFO : `internal/tui/model.go:1197-1222` ;
- historique : `internal/tui/history.go:42-98`.

Elle execute une seule action a la fois.

Chaque execution recoit un compteur local `uint64` :

```go
m.nextRunID++
```

Voir `internal/tui/model.go:1203-1206`.

Ce `runID` :

- existe seulement dans la TUI ;
- recommence a chaque lancement ;
- n'est pas inclus dans les evenements ;
- n'est pas persiste ;
- n'est pas globalement unique.

La TUI est donc un client asynchrone du dispatcher. Elle n'est pas un executor reutilisable.

## Garanties Reelles

Le systeme garantit actuellement :

- des requetes et resultats Go types ;
- un handler unique par `ActionID` ;
- un registre utilisable par plusieurs goroutines ;
- une file FIFO dans une instance TUI ;
- une seule action active par instance TUI ;
- une propagation cooperative du contexte ;
- des evenements et des demandes de saisie abstraits ;
- une ecriture atomique de `task.json`.

La TUI annule son contexte global quand elle se ferme :

- `internal/tui/run.go:72-84`

Cependant, l'action doit respecter ce contexte dans toutes ses dependances.

Il n'existe aucune garantie interprocessus. Une TUI, une CLI et un futur serveur peuvent modifier simultanement le meme workspace.

## Pause Et Annulation

### Pause

La pause n'existe pas.

Une demande de saisie suspend techniquement le handler. Ce n'est pas une pause durable :

- le handler reste bloque ;
- sa goroutine reste active ;
- aucun etat `waiting-input` n'est persiste ;
- aucun token de reprise n'existe ;
- un redemarrage perd l'execution.

Une pause arbitraire est difficile pour les actions qui utilisent Git, HTTP ou des processus externes.

Je recommande de ne pas promettre une pause generale.

Definissez plutot :

- `waiting-input` pour une saisie ;
- des points de controle pour quelques actions compatibles ;
- une reprise explicite depuis un point de controle persistant.

### Annulation

L'annulation existe seulement via `context.Context`.

Il manque :

- `Cancel(executionID)` ;
- un bouton d'annulation TUI ;
- le retrait d'une action en file ;
- les statuts `canceling` et `canceled` ;
- une annulation CLI propre sur SIGINT ;
- une politique sur les effets deja produits.

Dans la TUI, `Ctrl+C` ferme toute l'application :

- `internal/tui/model.go:366-373`

Le modal de progression ignore les autres touches :

- `internal/tui/model.go:407-408`

L'utilisateur ne peut donc pas annuler une action sans fermer la TUI.

## Principales Limites

### 1. Aucun identifiant d'execution durable

`ActionID` identifie le type d'action.

Il faut au minimum separer :

```text
ActionID      workspace.finish
ExecutionID   identifiant global de l'execution
AttemptID     tentative apres reprise eventuelle
```

Tous les evenements, prompts, resultats et erreurs doivent contenir `ExecutionID`.

### 2. Aucun cycle de vie commun

Les statuts TUI sont seulement :

- `running` ;
- `succeeded` ;
- `failed`.

Voir `internal/tui/history.go:26-32`.

Un executor partage necessite au minimum :

```text
queued
running
waiting-input
canceling
canceled
succeeded
failed
```

### 3. Aucune persistance des executions

L'historique TUI garde 20 executions et 160 evenements par execution.

Il disparait a la fermeture :

- `internal/tui/history.go:9-12`
- `internal/tui/history.go:64-98`

Un serveur web doit persister :

- la requete ;
- le statut ;
- les evenements ;
- les prompts ;
- les reponses ;
- le resultat ;
- l'erreur ;
- les dates ;
- l'identite de l'utilisateur.

### 4. Evenements non uniformes

Le contrat prevoit plusieurs types d'evenement :

- `started` ;
- `progress` ;
- `input-required` ;
- `completed` ;
- `warning` ;
- `log`.

Voir `internal/action/types.go:103-123`.

En pratique :

- le dispatcher ne produit pas automatiquement le debut et la fin ;
- les actions `workapp` produisent seulement `progress` ;
- leurs sequences restent a zero ;
- seule la mise a jour utilise reellement une sequence ;
- `Data any` ne forme pas un contrat JSON stable.

Un client web ne peut pas reconstruire fiablement l'ordre et le cycle de vie.

### 5. Resultats partiels perdus

Certains services produisent un rapport partiel avec une erreur :

- `internal/workapp/lifecycle.go:105-137`

Le dispatcher supprime toujours le resultat si une erreur existe :

- `internal/action/dispatcher.go:100-103`

Un client ne peut donc pas savoir quels effets ont deja reussi.

Cela pose un probleme important pour :

- les taches enfant deja creees ;
- les branches deja poussees ;
- les pull requests deja creees ;
- les etats distants deja modifies.

### 6. Effets non transactionnels

`workspace start` produit des effets locaux, puis des effets distants.

`workspace finish` pousse les branches, puis cree les pull requests, puis modifie les work items :

- `internal/workapp/finish.go:54-145`

Une erreur intermediaire laisse un etat partiel.

Une transaction globale n'est pas realiste avec Git et les fournisseurs distants.

Il faut donc utiliser :

- l'idempotence ;
- un journal d'etapes ;
- des resultats partiels ;
- des operations reprenables ;
- des compensations ciblees.

### 7. Aucune exclusion mutuelle metier

L'ecriture atomique du manifeste empeche un fichier partiellement ecrit :

- `internal/workspace/manifest.go:118-160`

Elle n'empeche pas une mise a jour perdue.

Deux processus peuvent lire la meme version, la modifier, puis ecraser leurs changements.

Il faut un verrou interprocessus par workspace pour les actions mutantes.

### 8. Contrats difficiles a transporter sur HTTP

Les requetes et resultats utilisent des interfaces Go concretes.

`EventEnvelope.Data` utilise `any`.

Pour un transport web, il faut :

- un discriminant stable ;
- un schema versionne ;
- un codec par `ActionID` ;
- des DTO JSON explicites ;
- une validation stricte des entrees.

Les reponses aux prompts sont actuellement peu validees :

- seul `Response.Kind` est controle ;
- les choix recus ne sont pas compares aux choix autorises ;
- `Required` n'est pas applique par le runtime.

Voir `internal/action/runtime.go:45-60`.

## Problemes Concrets A Corriger

Priorite haute :

1. Ajoutez `signal.NotifyContext` a la CLI.
2. Ajoutez un verrou interprocessus par workspace.
3. Conservez les resultats partiels avec les erreurs.
4. Diffusez correctement les evenements de `Finish`.
5. Rendez l'envoi final TUI sensible a l'annulation.

`Finish` recoit un `sink`, mais passe `nil` a l'execution locale :

- `internal/workapp/finish.go:11`
- `internal/workapp/finish.go:54`

Sa progression n'arrive donc pas reellement en direct.

L'envoi final de la goroutine TUI peut aussi rester bloque :

- `internal/tui/run.go:322-324`

Cet envoi doit selectionner egalement `ctx.Done()`.

## Architecture Web Recommandee

Conservez le dispatcher comme frontiere metier.

Ajoutez un composant `Executor` au-dessus :

```text
CLI / TUI / HTTP
       |
       v
Executor
  - Submit
  - Get
  - Cancel
  - Respond
  - Subscribe
       |
       v
Dispatcher
       |
       v
Application services
```

API conceptuelle :

```go
type Executor interface {
	Submit(context.Context, RequestEnvelope) (Execution, error)
	Get(context.Context, ExecutionID) (Execution, error)
	Cancel(context.Context, ExecutionID) error
	Respond(context.Context, ExecutionID, PromptID, Response) error
	Subscribe(context.Context, ExecutionID, uint64) (<-chan Event, error)
}
```

L'executor doit posseder :

- la file ;
- les identifiants d'execution ;
- les statuts ;
- les contextes d'annulation ;
- les sequences d'evenements ;
- la persistance ;
- les verrous metier ;
- la reprise des flux ;
- les controles d'acces.

Le web peut ensuite utiliser :

- `POST /executions` ;
- `GET /executions/{id}` ;
- `POST /executions/{id}/cancel` ;
- `POST /executions/{id}/responses/{promptID}` ;
- SSE ou WebSocket pour les evenements.

Une sequence persistee permet au client de se reconnecter :

```text
GET /executions/{id}/events?after=42
```

## Recommandation De Migration

### Etape 1

Corrigez l'execution locale :

- SIGINT ;
- annulation TUI ciblee ;
- verrou par workspace ;
- resultat partiel ;
- evenements coherents.

### Etape 2

Introduisez `ExecutionID` et le cycle de vie.

Utilisez d'abord un executor en memoire. Faites utiliser cet executor par la TUI.

Cela retirera la file et le `runID` du modele TUI.

### Etape 3

Versionnez les DTO et les evenements.

Remplacez `Data any` aux frontieres reseau.

Ajoutez une sequence a chaque evenement.

### Etape 4

Ajoutez la persistance et l'API web.

Ajoutez ensuite :

- reconnexion ;
- idempotency keys ;
- authentification ;
- autorisation ;
- protection des secrets.

## Tests

Les tests suivants passent avec CGO desactive :

```text
CGO_ENABLED=0 go test ./internal/action ./internal/cli/controller ./internal/tui
```

Le paquet `internal/action` ne contient aucun test.

Les tests prioritaires sont :

- dispatcher et erreurs de contrat ;
- annulation SIGINT ;
- annulation TUI active et en file ;
- ordre des evenements ;
- canal TUI sature ;
- ecritures concurrentes du manifeste ;
- restitution d'un resultat partiel ;
- reprise depuis une sequence d'evenement ;
- validation complete des reponses aux prompts.

## Conclusion

La base est adaptee au partage du metier entre CLI, TUI et web.

Le dispatcher et le `Runtime` forment une bonne frontiere.

Cependant, l'executor suppose n'existe pas encore comme composant partage. La TUI implemente actuellement une version locale et limitee de ce role.

Pour le web, il faut surtout ajouter une couche d'orchestration durable. Il n'est pas necessaire de reecrire les actions metier.

## Decision Livree

La decision [Execution and Local Web Architecture](architecture/011-execution-and-web.md) remplace les recommandations de conception de cet audit.

L'audit reste la preuve de l'etat initial et des priorites de migration.
