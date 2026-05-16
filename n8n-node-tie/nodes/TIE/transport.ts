import type { IDataObject, IExecuteFunctions } from 'n8n-workflow';
import { NodeApiError } from 'n8n-workflow';

import { normalizeBaseUrl } from './validation';

export interface TieApiOptions {
	method?: 'GET' | 'POST';
	path: string;
	body?: IDataObject;
	query?: IDataObject;
}

export async function tieApiRequest(
	executeFunctions: IExecuteFunctions,
	itemIndex: number,
	options: TieApiOptions,
): Promise<IDataObject> {
	const credentials = await executeFunctions.getCredentials('tieApi');
	const baseUrl = normalizeBaseUrl(String(credentials.baseUrl), executeFunctions, itemIndex);
	const timeoutMs = Number(credentials.timeoutMs ?? 5000);

	try {
		const response = await executeFunctions.helpers.httpRequest({
			baseURL: baseUrl,
			url: options.path,
			method: options.method ?? 'POST',
			body: options.body,
			qs: options.query,
			json: true,
			timeout: timeoutMs,
			headers: {
				accept: 'application/json',
			},
		});

		if (response === null || typeof response !== 'object' || Array.isArray(response)) {
			return {
				data: response as unknown as IDataObject,
			};
		}

		return response as IDataObject;
	} catch (error) {
		throw new NodeApiError(executeFunctions.getNode(), error as any, {
			itemIndex,
			message: 'Request to the TIE service failed.',
			description: 'Check the TIE base URL, API key, timeout, and service availability.',
		});
	}
}
