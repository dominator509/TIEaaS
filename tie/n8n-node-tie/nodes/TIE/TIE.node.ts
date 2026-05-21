import type {
	IDataObject,
	IExecuteFunctions,
	INodeExecutionData,
	INodeProperties,
	INodeType,
	INodeTypeDescription,
} from 'n8n-workflow';

import { buildValidationRequest } from './validation';
import { tieApiRequest } from './transport';

const targetTypeOptions: INodeProperties = {
	displayName: 'Target Type',
	name: 'targetType',
	type: 'options',
	default: 'code',
	options: [
		{ name: 'Code', value: 'code' },
		{ name: 'Fact', value: 'fact' },
		{ name: 'Action', value: 'action' },
	],
	description: 'What kind of subject to validate.',
};

export class TIE implements INodeType {
	description: INodeTypeDescription = {
		displayName: 'TIE Validator',
		name: 'tieValidator',
		icon: 'file:tie.svg',
		group: ['transform'],
		version: 1,
		subtitle: '={{$parameter["targetType"]}}',
		description: 'Send code, fact, or action payloads to the TIE validation service.',
		defaults: {
			name: 'TIE Validator',
		},
		inputs: ['main'],
		outputs: ['main'],
		credentials: [
			{
				name: 'tieApi',
				required: true,
			},
		],
		properties: [
			{
				displayName: 'Operation',
				name: 'operation',
				type: 'options',
				default: 'validate',
				options: [
					{ name: 'Validate', value: 'validate' },
					{ name: 'Health Check', value: 'health' },
				],
			},
			targetTypeOptions,
			{
				displayName: 'Enforcement Mode',
				name: 'enforcementMode',
				type: 'options',
				default: 'inherit',
				options: [
					{ name: 'Inherit Service Default', value: 'inherit' },
					{ name: 'Advisory', value: 'advisory' },
					{ name: 'Critical Fail Closed', value: 'critical_fail_closed' },
					{ name: 'Full Fail Closed', value: 'full_fail_closed' },
				],
				displayOptions: {
					show: {
						operation: ['validate'],
					},
				},
			},
			{
				displayName: 'Code Content',
				name: 'codeContent',
				type: 'string',
				default: '',
				typeOptions: {
					rows: 8,
				},
				displayOptions: {
					show: {
						operation: ['validate'],
						targetType: ['code'],
					},
				},
			},
			{
				displayName: 'Language',
				name: 'language',
				type: 'string',
				default: 'rust',
				displayOptions: {
					show: {
						operation: ['validate'],
						targetType: ['code'],
					},
				},
			},
			{
				displayName: 'Fact Claim',
				name: 'factClaim',
				type: 'string',
				default: '',
				typeOptions: {
					rows: 4,
				},
				displayOptions: {
					show: {
						operation: ['validate'],
						targetType: ['fact'],
					},
				},
			},
			{
				displayName: 'Action Type',
				name: 'actionType',
				type: 'string',
				default: '',
				displayOptions: {
					show: {
						operation: ['validate'],
						targetType: ['action'],
					},
				},
			},
			{
				displayName: 'Action Payload (JSON)',
				name: 'actionPayload',
				type: 'string',
				default: '{}',
				typeOptions: {
					rows: 8,
				},
				displayOptions: {
					show: {
						operation: ['validate'],
						targetType: ['action'],
					},
				},
			},
			{
				displayName: 'Evidence (JSON Array)',
				name: 'evidenceJson',
				type: 'string',
				default: '',
				description: 'Optional array of evidence objects to pass through to TIE.',
				typeOptions: {
					rows: 6,
				},
				displayOptions: {
					show: {
						operation: ['validate'],
					},
				},
			},
			{
				displayName: 'Context (JSON)',
				name: 'contextJson',
				type: 'string',
				default: '',
				typeOptions: {
					rows: 6,
				},
				description: 'Optional context object, such as workflow metadata or execution hints.',
				displayOptions: {
					show: {
						operation: ['validate'],
					},
				},
			},
			{
				displayName: 'Metadata (JSON)',
				name: 'metadataJson',
				type: 'string',
				default: '',
				typeOptions: {
					rows: 6,
				},
				displayOptions: {
					show: {
						operation: ['validate'],
					},
				},
			},
			{
				displayName: 'Webhook Callback URL',
				name: 'webhookCallbackUrl',
				type: 'string',
				default: '',
				description: 'Optional callback URL if TIE should deliver asynchronous validation updates.',
				displayOptions: {
					show: {
						operation: ['validate'],
					},
				},
			},
			{
				displayName: 'Webhook Event Types',
				name: 'webhookEventTypes',
				type: 'string',
				default: 'validation.completed',
				description: 'Comma-separated event types to subscribe to for callbacks.',
				displayOptions: {
					show: {
						operation: ['validate'],
					},
				},
			},
		],
	};

	async execute(this: IExecuteFunctions): Promise<INodeExecutionData[][]> {
		const items = this.getInputData();
		const returnData: INodeExecutionData[] = [];

		for (let itemIndex = 0; itemIndex < items.length; itemIndex += 1) {
			const operation = this.getNodeParameter('operation', itemIndex) as 'validate' | 'health';

			if (operation === 'health') {
				const health = await tieApiRequest(this, itemIndex, {
					method: 'GET',
					path: '/readyz',
				});
				returnData.push({ json: health });
				continue;
			}

			const requestBody = buildValidationRequest(this, itemIndex);
			const response = await tieApiRequest(this, itemIndex, {
				method: 'POST',
				path: '/v1/validate',
				body: requestBody,
			});

			const output: IDataObject = {
				request: requestBody,
				response,
				decision: response.decision ?? null,
				status: response.status ?? response.decision ?? 'unknown',
			};
			returnData.push({ json: output, pairedItem: { item: itemIndex } });
		}

		return [returnData];
	}
}
