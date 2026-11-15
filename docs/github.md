# GitHub setup

Fixloop runs on GitHub as a **GitHub App**. It asks for the smallest set of permissions the Reviewer needs:

| Permission | Access | Why |
|---|---|---|
| Metadata | read | Required by every App |
| Contents | read | Read changed files, `.fixloop.yaml` and team rules |
| Pull requests | write | Post review comments |
| Checks | write | Report pass / warn / fail |

Events: `pull_request` (opened, updated, reopened, ready for review) and `issue_comment` (for `/fixloop review`). Issue filing in a later release will ask for `issues: write`. GitHub then asks installation owners to approve the change.

## 1. Create the App

```sh
fixloop github create-app --name my-fixloop        # add --org my-org for an organisation
```

This opens GitHub with the App's manifest filled in. Click **Create GitHub App**. Fixloop saves the private key and a webhook secret in `~/.config/fixloop/` (readable only by you) and prints the config to add:

```yaml
github:
  app_id: 123456
  private_key_path: /home/me/.config/fixloop/my-fixloop.private-key.pem
```

```sh
export FIXLOOP_GITHUB_WEBHOOK_SECRET=$(jq -r .webhook_secret ~/.config/fixloop/my-fixloop.json)
```

## 2. Install it

Install the App on the repositories Fixloop should review (`https://github.com/apps/<slug>/installations/new`). Choose **Only select repositories** to start small.

## 3. Try a review without a server

```sh
fixloop --config fixloop.yaml review --pr https://github.com/me/repo/pull/1          # dry run, posts nothing
fixloop --config fixloop.yaml review --pr https://github.com/me/repo/pull/1 --post   # publishes
```

## 4. Run the server for automatic reviews

```sh
fixloop --config fixloop.yaml serve
```

GitHub must be able to reach `POST /webhooks/github`. In the App's settings, set the **Webhook URL** to `https://<your-host>/webhooks/github`, set the **Webhook secret** to the value you exported, and tick **Active**. Webhooks start disabled when the App is created without a public URL.

Every delivery is checked against the webhook secret. A request with a bad signature is refused, and redelivered events are not reviewed twice.

To run it on Kubernetes, use the image `ghcr.io/bwalia/fixloop` and the manifests in [deploy/kubernetes](../deploy/kubernetes/README.md).

## Commands in pull requests

| Comment | Effect |
|---|---|
| `/fixloop review` | Review the PR again. Only people with write access can use it. |

## What a review uses

- The PR diff, PR title and description, and the full new version of changed files (for context). All of it is treated as untrusted data.
- `.fixloop.yaml` and `.fixloop/rules/*.md` from the **base** commit, so a pull request cannot weaken its own review.
- Secrets in the diff are never sent to a model. They are replaced with `[REDACTED:<kind>]`, and a committed secret is itself reported as a critical finding.
