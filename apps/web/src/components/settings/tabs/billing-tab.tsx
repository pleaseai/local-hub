"use client";

import type { UserSettings } from "@/lib/user-settings-store";

interface BillingTabProps {
	settings: UserSettings;
	onNavigate: (tab: "general" | "editor" | "ai" | "billing" | "account") => void;
}

export function BillingTab(_props: BillingTabProps) {
	return (
		<div className="px-4 py-6">
			<p className="text-sm text-muted-foreground font-mono">
				Local mode — no billing configured.
			</p>
		</div>
	);
}
