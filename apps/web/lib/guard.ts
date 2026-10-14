import "server-only";
import { notFound } from "next/navigation";
import { TidelineError } from "@tideline/sdk";

/** Resolve an API call, turning a 404 (or a malformed id) into the not-found page. */
export async function orNotFound<T>(call: Promise<T>): Promise<T> {
  try {
    return await call;
  } catch (error) {
    if (error instanceof TidelineError && (error.status === 404 || error.status === 400)) {
      notFound();
    }
    throw error;
  }
}
