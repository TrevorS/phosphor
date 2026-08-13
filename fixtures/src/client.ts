// fixtures/src/client.ts — TypeScript fixture (T037's first-class twelve).

export interface RetryPolicy {
    maxAttempts: number;
    baseDelayMs: number;
    maxDelayMs: number;
}

export async function retryWithBackoff<T>(
    policy: RetryPolicy,
    attempt: () => Promise<T>,
): Promise<T> {
    let delay = policy.baseDelayMs;
    for (let tries = 0; tries < policy.maxAttempts; tries += 1) {
        try {
            return await attempt();
        } catch (error) {
            if (tries + 1 === policy.maxAttempts) {
                throw error;
            }
            await sleep(delay);
            delay = Math.min(delay * 2, policy.maxDelayMs);
        }
    }
    throw new Error("unreachable");
}

function sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
}
