import { type LayoutInput, type LayoutResult, computeLayout } from "./layout";
import type { LayoutRequest, LayoutResponse } from "./layout.worker";

/** A layout computation in progress. */
export interface LayoutTask {
	/** Resolves with the layout; never resolves after `cancel()` */
	promise: Promise<LayoutResult>;
	/** Abandons the computation (terminates the worker if one is running) */
	cancel: () => void;
}

/**
 * Computes the layout on the main thread on the next macrotask, so the "laying out" overlay can paint first.
 * @param input - Graph to lay out
 * @param isCancelled - Returns true when the caller no longer wants the result
 * @returns Promise resolving with the layout
 */
function runOnMainThread(input: LayoutInput, isCancelled: () => boolean): Promise<LayoutResult> {
	return new Promise(function deferLayout(resolve, reject) {
		setTimeout(
			/** Computes the layout unless the task was cancelled meanwhile. */
			function computeDeferred() {
				if (isCancelled()) return;
				try {
					resolve(computeLayout(input));
				} catch (error) {
					reject(error instanceof Error ? error : new Error(String(error)));
				}
			},
			0,
		);
	});
}

/**
 * Creates a dedicated module worker for one layout run, or null if workers are unavailable.
 * @returns A new worker, or null
 */
function createLayoutWorker(): Worker | null {
	if (typeof Worker === "undefined") return null;
	try {
		return new Worker(new URL("./layout.worker.ts", import.meta.url), { type: "module" });
	} catch {
		return null;
	}
}

/**
 * Starts a layout computation off the main thread in a Web Worker.
 * Each run uses its own worker so a stale run can be terminated immediately when the data or grouping
 * changes. Falls back to a (deferred) main-thread computation if workers are unavailable or fail.
 * @param input - Graph to lay out (its typed arrays are copied, not transferred)
 * @returns A cancellable layout task
 */
export function runLayout(input: LayoutInput): LayoutTask {
	let cancelled = false;
	let worker: Worker | null = createLayoutWorker();

	/** Reports whether the task was cancelled. */
	function isCancelled(): boolean {
		return cancelled;
	}

	/** Terminates and forgets the worker, if any. */
	function disposeWorker(): void {
		worker?.terminate();
		worker = null;
	}

	const promise = new Promise<LayoutResult>(function startLayout(resolve, reject) {
		/**
		 * Falls back to the main thread after a worker failure.
		 * @param reason - Why the worker could not be used (logged as a warning)
		 */
		function fallBack(reason: string): void {
			disposeWorker();
			if (cancelled) return;
			console.warn(`Layout worker unavailable (${reason}); laying out on the main thread.`);
			runOnMainThread(input, isCancelled).then(resolve, reject);
		}

		if (!worker) {
			runOnMainThread(input, isCancelled).then(resolve, reject);
			return;
		}

		/**
		 * Resolves (or rejects) with the worker's response and disposes the worker.
		 * @param event - Message from the layout worker
		 */
		worker.onmessage = function handleWorkerMessage(event: MessageEvent<LayoutResponse>) {
			const response = event.data;
			disposeWorker();
			if (cancelled) return;
			if (response.ok) {
				resolve(response.result);
			} else {
				reject(new Error(response.error));
			}
		};
		/**
		 * Falls back to the main thread when the worker fails to load or crashes.
		 * @param event - Worker error event
		 */
		worker.onerror = function handleWorkerError(event: ErrorEvent) {
			event.preventDefault();
			fallBack(event.message || "worker error");
		};

		const request: LayoutRequest = { input };
		worker.postMessage(request);
	});

	return {
		promise,
		/** Abandons the computation and terminates the worker, if any. */
		cancel() {
			cancelled = true;
			disposeWorker();
		},
	};
}
