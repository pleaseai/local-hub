import { createOpenRouter } from "@openrouter/ai-sdk-provider";

export type AIProviderType = "openrouter" | "claude-code" | "gemini-cli" | "codex-cli";

/**
 * Create an AI SDK provider instance based on the provider type.
 *
 * - openrouter: Uses OpenRouter API with the given API key
 * - claude-code: Uses local Claude Code CLI via ai-sdk-provider-claude-code
 * - gemini-cli: Uses local Gemini CLI via ai-sdk-provider-gemini-cli
 * - codex-cli: Uses local Codex CLI via ai-sdk-provider-codex-cli
 */
export function createAIProvider(
	providerType: AIProviderType,
	options: { apiKey?: string } = {},
) {
	switch (providerType) {
		case "openrouter":
			return createOpenRouter({ apiKey: options.apiKey });

		case "claude-code": {
			// Dynamic import to avoid requiring CLI providers when not used
			const { createClaudeCode } = require("ai-sdk-provider-claude-code");
			return createClaudeCode();
		}

		case "gemini-cli": {
			const { createGeminiCli } = require("ai-sdk-provider-gemini-cli");
			return createGeminiCli();
		}

		case "codex-cli": {
			const { createCodexCli } = require("ai-sdk-provider-codex-cli");
			return createCodexCli();
		}

		default:
			return createOpenRouter({ apiKey: options.apiKey });
	}
}

/**
 * Detect the provider type from a model ID string.
 * CLI providers use special prefixes; everything else goes through OpenRouter.
 */
export function detectProviderType(modelId: string): AIProviderType {
	if (modelId.startsWith("claude-code:")) return "claude-code";
	if (modelId.startsWith("gemini-cli:")) return "gemini-cli";
	if (modelId.startsWith("codex-cli:")) return "codex-cli";
	return "openrouter";
}

/**
 * Strip the provider prefix from a model ID.
 * e.g., "claude-code:claude-4-sonnet" → "claude-4-sonnet"
 */
export function stripProviderPrefix(modelId: string): string {
	const colonIndex = modelId.indexOf(":");
	if (colonIndex === -1) return modelId;

	const prefix = modelId.substring(0, colonIndex);
	if (["claude-code", "gemini-cli", "codex-cli"].includes(prefix)) {
		return modelId.substring(colonIndex + 1);
	}
	return modelId;
}

/**
 * Check if a provider type is a local CLI provider.
 */
export function isLocalProvider(providerType: AIProviderType): boolean {
	return providerType !== "openrouter";
}
