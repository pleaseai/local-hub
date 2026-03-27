import { PrismaClient } from "../generated/prisma/client";

const globalForPrisma = globalThis as unknown as { prisma: PrismaClient | undefined };

function makePrisma() {
	return new PrismaClient();
}

export const prisma: PrismaClient = globalForPrisma.prisma ?? (globalForPrisma.prisma = makePrisma());
