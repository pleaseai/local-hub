/**
 * Local mode: always allow AI usage (no billing).
 */
export async function checkUsageLimit(
	_userId: string,
	_isCustomApiKey = false,
): Promise<{
	allowed: boolean;
	current: number;
	limit: number;
	creditExhausted?: boolean;
	spendingLimitReached?: boolean;
}> {
	return { allowed: true, current: 0, limit: 0 };
}
