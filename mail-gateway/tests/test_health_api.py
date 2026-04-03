from fastapi.testclient import TestClient

from mail_gateway.app import create_app
from mail_gateway.config import Settings


def test_health_endpoint_reports_enabled_luckmail_and_yyds_providers() -> None:
    settings = Settings(
        host='127.0.0.1',
        port=8081,
        database_path=':memory:',
        luckmail_base_url='https://mails.luckyous.com',
        luckmail_api_key='AC-test-key',
        yyds_base_url='https://maliapi.215.im/v1',
        yyds_api_key='AC-yyds-test-key',
    )
    client = TestClient(create_app(settings=settings))

    response = client.get('/health')

    assert response.status_code == 200
    payload = response.json()
    assert payload['status'] == 'ok'
    assert payload['providers']['luckmail'] == 'enabled'
    assert payload['providers']['yyds_mail'] == 'enabled'
    assert payload['providers']['duckmail'] == 'disabled'
    assert isinstance(payload['timestamp'], int)


def test_health_endpoint_reports_disabled_providers_when_keys_missing() -> None:
    settings = Settings(
        host='127.0.0.1',
        port=8081,
        database_path=':memory:',
        luckmail_base_url='https://mails.luckyous.com',
        luckmail_api_key='',
        yyds_base_url='https://maliapi.215.im/v1',
        yyds_api_key='',
    )
    client = TestClient(create_app(settings=settings))

    response = client.get('/health')

    assert response.status_code == 200
    payload = response.json()
    assert payload['status'] == 'ok'
    assert payload['providers']['luckmail'] == 'disabled'
    assert payload['providers']['yyds_mail'] == 'disabled'
    assert payload['providers']['duckmail'] == 'disabled'
    assert isinstance(payload['timestamp'], int)
