import { NodeOperationError, type IDataObject, type IExecuteFunctions } from 'n8n-workflow';

export type ValidationTarget = 'code' | 'fact' | 'action';
export type EnforcementMode = 'inherit' | 'advisory' | 'critical_fail_closed' | 'full_fail_closed';

export interface TieValidationRequest extends IDataObject {
	request_id?: string;
	target_type: ValidationTarget;
	enforcement_mode?: Exclude<EnforcementMode, 'inherit'>;
	subject: {
		content?: string;
		language?: string;
		claim?: string;
		action_type?: string;
		payload?: IDataObject;
	};
	evidence?: IDataObject[];
	context?: IDataObject;
	metadata?: IDataObject;
	webhook?: {
		callback_url: string;
		event_types?: string[];
	};
}

const urlPattern = /^https?:\/\//i;

export function parseJsonInput(executeFunctions: IExecuteFunctions, itemIndex: number, fieldName: string): IDataObject | undefined {
	const rawValue = executeFunctions.getNodeParameter(fieldName, itemIndex, '') as string;
	if (!rawValue) {
		return undefined;
	}

	try {
		const parsed = JSON.parse(rawValue);
		if (parsed === null || Array.isArray(parsed) || typeof parsed !== 'object') {
			throw new Error('Expected a JSON object');
		}
		return parsed as IDataObject;
	} catch (error) {
		throw new NodeOperationError(
			executeFunctions.getNode(),
			`Field "${fieldName}" must contain a valid JSON object. ${(error as Error).message}`,
			{ itemIndex },
		);
	}
}

export function parseJsonArrayInput(
	executeFunctions: IExecuteFunctions,
	itemIndex: number,
	fieldName: string,
): IDataObject[] | undefined {
	const rawValue = executeFunctions.getNodeParameter(fieldName, itemIndex, '') as string;
	if (!rawValue) {
		return undefined;
	}

	try {
		const parsed = JSON.parse(rawValue);
		if (!Array.isArray(parsed)) {
			throw new Error('Expected a JSON array');
		}
		return parsed.map((entry, index) => {
			if (entry === null || Array.isArray(entry) || typeof entry !== 'object') {
				throw new Error(`Entry at index ${index} is not a JSON object`);
			}
			return entry as IDataObject;
		});
	} catch (error) {
		throw new NodeOperationError(
			executeFunctions.getNode(),
			`Field "${fieldName}" must contain a valid JSON array of objects. ${(error as Error).message}`,
			{ itemIndex },
		);
	}
}

export function normalizeBaseUrl(baseUrl: string, executeFunctions: IExecuteFunctions, itemIndex: number): string {
	const normalized = baseUrl.trim().replace(/\/+$/, '');
	if (!normalized || !urlPattern.test(normalized)) {
		throw new NodeOperationError(
			executeFunctions.getNode(),
			'Credential field "Base URL" must be a valid http or https URL.',
			{ itemIndex },
		);
	}
	return normalized;
}

export function buildValidationRequest(
	executeFunctions: IExecuteFunctions,
	itemIndex: number,
): TieValidationRequest {
	const targetType = executeFunctions.getNodeParameter('targetType', itemIndex) as ValidationTarget;
	const enforcementMode = executeFunctions.getNodeParameter('enforcementMode', itemIndex, 'inherit') as EnforcementMode;
	const webhookCallbackUrl = executeFunctions.getNodeParameter('webhookCallbackUrl', itemIndex, '') as string;
	const webhookEventTypes = executeFunctions.getNodeParameter('webhookEventTypes', itemIndex, '') as string;
	const language = executeFunctions.getNodeParameter('language', itemIndex, '') as string;
	const codeContent = executeFunctions.getNodeParameter('codeContent', itemIndex, '') as string;
	const factClaim = executeFunctions.getNodeParameter('factClaim', itemIndex, '') as string;
	const actionType = executeFunctions.getNodeParameter('actionType', itemIndex, '') as string;
	const actionPayload = parseJsonInput(executeFunctions, itemIndex, 'actionPayload');
	const context = parseJsonInput(executeFunctions, itemIndex, 'contextJson');
	const metadata = parseJsonInput(executeFunctions, itemIndex, 'metadataJson');
	const evidence = parseJsonArrayInput(executeFunctions, itemIndex, 'evidenceJson');

	const subject: TieValidationRequest['subject'] = {};

	if (targetType === 'code') {
		if (!codeContent.trim()) {
			throw new NodeOperationError(executeFunctions.getNode(), 'Code content is required for code validation.', {
				itemIndex,
			});
		}
		subject.content = codeContent;
		if (language.trim()) {
			subject.language = language.trim();
		}
	}

	if (targetType === 'fact') {
		if (!factClaim.trim()) {
			throw new NodeOperationError(executeFunctions.getNode(), 'Fact claim is required for fact validation.', {
				itemIndex,
			});
		}
		subject.claim = factClaim;
	}

	if (targetType === 'action') {
		if (!actionType.trim()) {
			throw new NodeOperationError(executeFunctions.getNode(), 'Action type is required for action validation.', {
				itemIndex,
			});
		}
		if (!actionPayload) {
			throw new NodeOperationError(executeFunctions.getNode(), 'Action payload is required for action validation.', {
				itemIndex,
			});
		}
		subject.action_type = actionType.trim();
		subject.payload = actionPayload;
	}

	const request: TieValidationRequest = {
		target_type: targetType,
		subject,
	};

	if (enforcementMode !== 'inherit') {
		request.enforcement_mode = enforcementMode;
	}
	if (context) {
		request.context = context;
	}
	if (metadata) {
		request.metadata = metadata;
	}
	if (evidence && evidence.length > 0) {
		request.evidence = evidence;
	}
	if (webhookCallbackUrl.trim()) {
		if (!urlPattern.test(webhookCallbackUrl.trim())) {
			throw new NodeOperationError(
				executeFunctions.getNode(),
				'Webhook callback URL must be a valid http or https URL.',
				{ itemIndex },
			);
		}
		request.webhook = {
			callback_url: webhookCallbackUrl.trim(),
		};
		const events = webhookEventTypes
			.split(',')
			.map((value) => value.trim())
			.filter(Boolean);
		if (events.length > 0) {
			request.webhook.event_types = events;
		}
	}

	return request;
}
