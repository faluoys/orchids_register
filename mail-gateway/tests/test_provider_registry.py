from mail_gateway.config import Settings
from mail_gateway.providers.registry import build_providers


def test_build_providers_exposes_all_provider_keys() -> None:
    settings = Settings(
        host='127.0.0.1',
        port=8081,
        database_path=':memory:',
        luckmail_base_url='https://mails.luckyous.com',
        luckmail_api_key='AC-test-key',
        yyds_base_url='https://maliapi.215.im/v1',
        yyds_api_key='AC-yyds-test-key',
        mail_chatgpt_uk_base_url='https://mail.chatgpt.org.uk',
        mail_chatgpt_uk_api_key='AC-mail-chatgpt-uk-test-key',
    )

    providers = build_providers(settings=settings, testing=True)

    assert set(providers) == {'luckmail', 'yyds_mail', 'mail_chatgpt_uk', 'duckmail'}
    luckmail_acquired = providers['luckmail'].acquire_inbox('orchids', None, {})
    assert luckmail_acquired.address == 'user1@outlook.com'
    assert luckmail_acquired.upstream_token == 'tok_abc123'
    assert luckmail_acquired.upstream_ref == 'purchase:1'

    yyds_acquired = providers['yyds_mail'].acquire_inbox('orchids', 'example.com', {'prefix': 'orchids'})
    assert yyds_acquired.address == 'orchids@example.com'
    assert yyds_acquired.upstream_token == 'orchids@example.com'
    assert yyds_acquired.upstream_ref == 'inbox:ibox_stub'

    mail_chatgpt_uk_acquired = providers['mail_chatgpt_uk'].acquire_inbox(
        'orchids',
        'chatgpt.org.uk',
        {'prefix': 'orchids'},
    )
    assert mail_chatgpt_uk_acquired.address == 'orchids@chatgpt.org.uk'
    assert mail_chatgpt_uk_acquired.upstream_token == 'orchids@chatgpt.org.uk'
    assert mail_chatgpt_uk_acquired.upstream_ref == 'inbox:mail_chatgpt_uk_stub'
