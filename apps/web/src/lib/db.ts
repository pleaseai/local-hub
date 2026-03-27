import path from "node:path";
import { PrismaClient } from "../generated/prisma/client";

const globalForPrisma = globalThis as unknown as { prisma: InstanceType<typeof PrismaClient> | undefined };

const dbUrl = process.env.DATABASE_URL ?? `file:${path.resolve("prisma/dev.db")}`;

function makePrisma() {
	return new PrismaClient({
		datasourceUrl: dbUrl,
	} as any);
}

export const prisma = globalForPrisma.prisma ?? (globalForPrisma.prisma = makePrisma());
