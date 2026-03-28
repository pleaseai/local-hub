import { Octokit } from "@octokit/rest";
import { createConfiguredOctokit } from "@/lib/github-config";
import { getServerSession } from "@/lib/auth";

export async function getOctokitFromSession(): Promise<Octokit | null> {
	const token = await getGitHubToken();
	if (!token) return null;
	return createConfiguredOctokit(token);
}

export async function getGitHubToken(): Promise<string | null> {
	const session = await getServerSession();
	if (!session) return null;
	return session.githubUser.accessToken;
}
