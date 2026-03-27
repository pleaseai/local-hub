import { Octokit } from "@octokit/rest";

/**
 * Central GitHub API URL configuration.
 * Set GITHUB_API_URL to route all API calls through local-hub proxy.
 */
export const GITHUB_API_URL = process.env.GITHUB_API_URL || "https://api.github.com";
export const GITHUB_GRAPHQL_URL = process.env.GITHUB_GRAPHQL_URL || `${GITHUB_API_URL}/graphql`;

/**
 * Create an Octokit instance with configurable baseUrl.
 * When GITHUB_API_URL is set (e.g., http://localhost:8787),
 * all REST API calls will route through the local-hub proxy.
 */
export function createConfiguredOctokit(token: string): Octokit {
	return new Octokit({
		auth: token,
		...(GITHUB_API_URL !== "https://api.github.com" ? { baseUrl: GITHUB_API_URL } : {}),
	});
}
