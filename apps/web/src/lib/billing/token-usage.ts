import type { LanguageModelUsage } from "ai";
import { prisma } from "../db";

function buildUsageDetails(usage: LanguageModelUsage) {
	return {
		input: usage.inputTokens ?? 0,
		output: usage.outputTokens ?? 0,
		total: usage.totalTokens ?? 0,
	};
}

/**
 * Log AI token usage to the database (no billing in local mode).
 */
export async function logTokenUsage(params: {
	userId: string;
	provider: string;
	modelId: string;
	taskType: string;
	usage: LanguageModelUsage;
	isCustomApiKey: boolean;
	conversationId?: string | undefined;
}): Promise<void> {
	const usageDetails = buildUsageDetails(params.usage);

	await prisma.aiCallLog.create({
		data: {
			userId: params.userId,
			provider: params.provider,
			modelId: params.modelId,
			taskType: params.taskType,
			inputTokens: usageDetails.input,
			outputTokens: usageDetails.output,
			totalTokens: usageDetails.total,
			usingOwnKey: params.isCustomApiKey,
			conversationId: params.conversationId,
		},
	});
}

/**
 * Log fixed-cost usage (no-op in local mode).
 */
export async function logFixedCostUsage(_params: {
	userId: string;
	taskType: string;
	costUsd?: number | undefined;
}): Promise<void> {
	// No billing in local mode
}
