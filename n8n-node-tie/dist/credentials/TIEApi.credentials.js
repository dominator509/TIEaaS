"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.TIEApi = void 0;
class TIEApi {
    constructor() {
        this.name = 'tieApi';
        this.displayName = 'TIE API';
        this.documentationUrl = 'https://example.com/tie/docs/api_usage';
        this.properties = [
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
        this.test = {
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
        this.authenticate = {
            type: 'generic',
            properties: {
                headers: {
                    'X-API-Key': '={{$credentials.apiKey}}',
                },
            },
        };
    }
    preAuthentication(credentials) {
        const normalized = { ...credentials };
        if (typeof normalized.baseUrl === 'string') {
            normalized.baseUrl = normalized.baseUrl.replace(/\/+$/, '');
        }
        return Promise.resolve(normalized);
    }
}
exports.TIEApi = TIEApi;
