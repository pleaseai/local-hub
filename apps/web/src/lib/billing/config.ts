// ── Error Codes ──

export const BILLING_ERROR = {
	MESSAGE_LIMIT_REACHED: "MESSAGE_LIMIT_REACHED",
	CREDIT_EXHAUSTED: "CREDIT_EXHAUSTED",
	SPENDING_LIMIT_REACHED: "SPENDING_LIMIT_REACHED",
} as const;

export type BillingErrorCode = (typeof BILLING_ERROR)[keyof typeof BILLING_ERROR];

export function getBillingErrorCode(result: {
	creditExhausted?: boolean;
	spendingLimitReached?: boolean;
}): BillingErrorCode {
	if (result.creditExhausted) return BILLING_ERROR.CREDIT_EXHAUSTED;
	if (result.spendingLimitReached) return BILLING_ERROR.SPENDING_LIMIT_REACHED;
	return BILLING_ERROR.MESSAGE_LIMIT_REACHED;
}
