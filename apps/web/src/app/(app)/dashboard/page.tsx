import type { Metadata } from "next";
import { getServerSession } from "@/lib/auth";
import { redirect } from "next/navigation";
import {
	getUserRepos,
	searchIssues,
	getNotifications,
	getUserEvents,
	getTrendingRepos,
} from "@/lib/github";
import { DashboardContent } from "@/components/dashboard/dashboard-content";
import { all } from "better-all";

export const metadata: Metadata = {
	title: "Dashboard",
};

export default async function DashboardPage() {
	const session = await getServerSession();
	if (!session) return redirect("/");
	const { githubUser } = session;
	const login = githubUser.login as string | undefined;
	const { reviewRequests, myOpenPRs, myIssues, repos, notifications, activity, trending } =
		await all({
			reviewRequests: async () =>
				login
					? await searchIssues(
							`is:pr is:open review-requested:${login}`,
							10,
						)
					: [],
			myOpenPRs: async () =>
				login
					? await searchIssues(`is:pr is:open author:${login}`, 10)
					: [],
			myIssues: async () =>
				login
					? await searchIssues(
							`is:issue is:open assignee:${login}`,
							10,
						)
					: [],
			repos: async () => await getUserRepos("updated", 30),
			notifications: async () => await getNotifications(20),
			activity: async () => (login ? await getUserEvents(login, 20) : []),
			trending: async () => await getTrendingRepos(undefined, "weekly", 8),
		});

	return (
		<DashboardContent
			user={githubUser}
			reviewRequests={reviewRequests}
			myOpenPRs={myOpenPRs}
			myIssues={myIssues}
			repos={repos}
			notifications={notifications}
			activity={activity}
			trending={trending}
		/>
	);
}
