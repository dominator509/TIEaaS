# n8n Workflow Example

This guide shows how to use the custom TIE node in an n8n workflow to validate model or workflow outputs before the next automation step runs.

## Use case

A workflow generates a deployment recommendation, validates it with TIE, and only proceeds when the verdict is safe under the deployment policy.

## Required setup

1. Install the custom package from `n8n-node-tie/`.
2. Create TIE API credentials in n8n.
3. Set the TIE base URL and API key.
4. Ensure the TIE service is reachable from the n8n runtime.

## Example workflow shape

```text
Manual Trigger
  -> Set / Prepare Payload
  -> AI or Custom Logic Node
  -> TIE Validate Node
  -> IF verdict == pass
      -> Execute Action
    ELSE
      -> Notify / Create Review Ticket
```

## Example payload mapping

### Code validation

```json
{
  "artifact_type": "code",
  "artifact": {
    "language": "rust",
    "content": "fn deploy() { println!(\"ok\"); }"
  },
  "registry_refs": ["policy/code/default"],
  "metadata": {
    "source": "n8n",
    "workflowId": "{{$workflow.id}}",
    "executionId": "{{$execution.id}}"
  }
}
```

### Action validation

```json
{
  "artifact_type": "action",
  "artifact": {
    "action": "deploy_service",
    "target": "payments-api",
    "environment": "production",
    "changeTicket": "CHG-4211"
  },
  "registry_refs": ["runbooks/deploy/prod"],
  "metadata": {
    "source": "n8n",
    "workflowId": "{{$workflow.id}}",
    "executionId": "{{$execution.id}}"
  }
}
```

## Suggested IF node rules

### Strict mode

Proceed only when:

- `{{$json["verdict"]}} == "pass"`

### Gradual rollout mode

Proceed when:

- `{{$json["verdict"]}} == "pass"`
- or `{{$json["verdict"]}} == "warn"` and `{{$json["severity"]}} != "high"`

## Operational notes

- Record `request_id` from TIE in all downstream audit systems.
- Add a dead-letter or manual review branch for blocked outputs.
- Keep TIE validation close to the side-effecting node, not only near the generation node.
- Run health checks from n8n startup logic or credential tests against `/readyz`.

## Troubleshooting

### Node returns invalid input

Check that:

- `artifact_type` is one of `code`, `fact`, or `action`
- required fields are present in the artifact body
- registry references point to active records

### Node times out

Check that:

- TIE is reachable from the n8n network
- verifier budgets are not set too low for the workload
- webhooks or async orchestration are used for heavy validation paths
