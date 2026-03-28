import { createAuthClient } from "better-auth/react";
import { inferAdditionalFields } from "better-auth/client/plugins";
import { dashClient, sentinelClient } from "@better-auth/infra/client";
import { auth } from "./auth";

export const authClient = createAuthClient({
	plugins: [
		inferAdditionalFields<typeof auth>(),
		...(process.env.NEXT_PUBLIC_BETTER_AUTH_INFRA === "true"
			? [dashClient(), sentinelClient()]
			: []),
	],
});

export const { signIn, signOut, useSession } = authClient;
