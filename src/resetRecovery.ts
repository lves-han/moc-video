export interface ResetRecoveryInfo { message: string }
export function resetRecoveryInfo(): ResetRecoveryInfo | undefined {
  const value = (globalThis as typeof globalThis & { __MOC_RESET_RECOVERY__?: unknown }).__MOC_RESET_RECOVERY__;
  if (value && typeof value === 'object' && 'message' in value && typeof value.message === 'string') return { message: value.message };
}
