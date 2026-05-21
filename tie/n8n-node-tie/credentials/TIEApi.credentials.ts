import type {
	ICredentialDataDecryptedObject,
	ICredentialTestRequest,
	ICredentialType,
	INodeProperties,
} from 'n8n-workflow';

export class TIEApi implements ICredentialType {
	name = 'tieApi';
	displayName = 'TIE API';
	documentationUrl = 'https://example.com/tie/docs/api_usage';
	properties: INodeProperties[] = [
		{
			displayName: 'Base URL',
			name: 'baseUrl',
			type: 'string',
			default: 'http://localhost:8080',
			required: true,
			placeholder: 'https://tie.internal.example.com',
			description: 'Base URL of the TIE service. Do not include a trailing slash.',
		},
		{
			displayName: 'API Key',
			name: 'apiKey',
			type: 'string',
			typeOptions: {
				password: true,
			},
			default: '',
			required: true,
			description: 'TIE API key that will be sent as the X-API-Key header.',
		},
		{
			displayName: 'Timeout (ms)',
			name: 'timeoutMs',
			type: 'number',
			default: 5000,
			typeOptions: {
				minValue: 250,
				maxValue: 60000,
			},
			description: 'HTTP timeout for requests to the TIE service.',
		},
	];

	test: ICredentialTestRequest = {
		request: {
			baseURL: '={{$credentials.baseUrl}}',
			url: '/readyz',
			method: 'GET',
			headers: {
				'X-API-Key': '={{$credentials.apiKey}}',
			},
			timeout: 5000,
		},
	};

	authenticate = {
		type: 'generic' as const,
		properties: {
			headers: {
				'X-API-Key': '={{$credentials.apiKey}}',
			},
		},
	};

	preAuthentication?(credentials: ICredentialDataDecryptedObject): Promise<ICredentialDataDecryptedObject> {
		const normalized = { ...credentials };
		if (typeof normalized.baseUrl === 'string') {
			normalized.baseUrl = normalized.baseUrl.replace(/\/+$/, '');
		}
		return Promise.resolve(normalized);
	}
}
