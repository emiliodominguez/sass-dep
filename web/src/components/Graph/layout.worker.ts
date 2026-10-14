import { type LayoutInput, type LayoutResult, computeLayout } from "./layout";

/** Message sent from the main thread to the layout worker. */
export interface LayoutRequest {
	/** Graph to lay out */
	input: LayoutInput;
}

/** Message sent from the layout worker back to the main thread. */
export type LayoutResponse = { ok: true; result: LayoutResult } | { ok: false; error: string };

/** The subset of DedicatedWorkerGlobalScope used here (the DOM lib does not ship worker types). */
interface LayoutWorkerScope {
	/** Handler for incoming messages */
	onmessage: ((event: MessageEvent<LayoutRequest>) => void) | null;
	/** Posts a message to the main thread, transferring the given buffers */
	postMessage(message: LayoutResponse, transfer: Transferable[]): void;
}

const scope = self as unknown as LayoutWorkerScope;

/**
 * Runs the dagre layout for one request and posts the positions back, transferring typed arrays.
 * @param event - Message containing the layout request
 */
function handleMessage(event: MessageEvent<LayoutRequest>): void {
	try {
		const result = computeLayout(event.data.input);
		scope.postMessage({ ok: true, result }, [result.positions.buffer, result.groupOf.buffer]);
	} catch (error) {
		scope.postMessage({ ok: false, error: error instanceof Error ? error.message : String(error) }, []);
	}
}

scope.onmessage = handleMessage;
