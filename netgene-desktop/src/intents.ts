import { invoke } from '@tauri-apps/api/core';

export type Action = { kind: string; params: Record<string, unknown> };
export type DispatchOutcome =
  | { status: 'executed'; action: Action; explanation: string; result: unknown }
  | { status: 'pending_confirmation'; proposal_id: string; action: Action; reason: string; explanation: string }
  | { status: 'denied'; action: Action | null; reason: string; explanation: string };

/**
 * Propose an intent; if the policy holds it for confirmation, ask the
 * operator (naming the exact action) before approving or rejecting it.
 */
export async function dispatchWithConfirmation(intent: string): Promise<DispatchOutcome> {
  const outcome = await invoke<DispatchOutcome>('dispatch_intent', { intent });
  if (outcome.status !== 'pending_confirmation') return outcome;

  const approved = window.confirm(
    `Confirmar a ação "${outcome.action.kind}" ${JSON.stringify(outcome.action.params)}?\n\n${outcome.reason}`
  );
  if (!approved) {
    await invoke('reject_intent', { proposalId: outcome.proposal_id });
    return outcome;
  }
  return invoke<DispatchOutcome>('confirm_intent', { proposalId: outcome.proposal_id });
}
