# SRS — rusty_azure_devops

**Версия:** 0.1 (черновик)
**Дата:** 2025-12-20

---

## 1. Краткое описание проекта ✨

**Название:** rusty_azure_devops

**Назначение:** MCP-сервер (Model Context Protocol) на языке Rust (>= 1.92) для взаимодействия с локальным Azure DevOps Server (on‑prem) через REST API. Сервер предоставляет набор инструментов (tools) по stdio (rmcp) для базовой работы с Work Items (WI): создание, чтение (поля, ссылки, комментарии), обновление и поиск.

**Ограничения:**
- Аутентификация может быть опущена (предполагается доверенная среда), поддержка PAT конфигурируется опционально.
- Вложений (attachments) в первом релизе нет.
- Локальное постоянство/кэш не требуется.
- Поддерживается работа с любыми проектами, к которым есть доступ по API‑токену.

---

## 2. Цели и область применения 🎯

- Обеспечить удобный, расширяемый MCP-интерфейс для операций с Work Items Azure DevOps.
- Поддерживать интеграцию с внешними автоматизациями и LLM-агентами через stdio (rmcp).
- Фокус на корректности операций (atomicity, concurrency control via rev/test), ясных ошибках и предсказуемом поведении.

---

## 3. Терминология и определения 📚

- WI — Work Item в Azure DevOps (Bug, Task, User Story, Custom и др.).
- MCP — Model Context Protocol (rmcp implementation).
- rmcp — Rust crate для реализации MCP (поддерживает stdio transport, tools, schema validation и пр.).
- azclient — внутренняя абстракция HTTP-клиента для общения с Azure DevOps REST API.

## 3.1 Конфигурация — переменные окружения (обязательно)

**Все** параметры конфигурации задаются исключительно через переменные окружения (ENV). Ни одна команда MCP не изменяет или сохраняет конфигурацию между запусками — конфиг читается при старте из окружения.

Обязательные и рекомендованные переменные окружения:

- `ADO_TOKEN` (обязательно) — Personal Access Token (PAT) для доступа к Azure DevOps; без него сервер не запускается.
- `ADO_BASE_URL` (обязательно) — базовый URL Azure DevOps on‑prem или облака (например `https://dev.azure.com/org` или `https://ado-server/collection`). Для надёжной работы `ADO_BASE_URL` обязателен и должен быть установлен вместе с `ADO_TOKEN`.
- `ADO_ORGANIZATION` (опционально) — организация (используется вместе с `ADO_BASE_URL` при необходимости).
- `ADO_API_VERSION` (опционально) — версия REST API; по умолчанию используется **latest** (если поддерживается клиентской библиотекой) и может быть переопределена через ENV.
- `ADO_DEFAULT_PROJECT` (опционально) — если задан, используется как проект по умолчанию для команд, где `project` не передан.
- `RUST_LOG` — уровень логирования (`info`, `warn`, `debug`, `trace`).

Примечание: конфигурация может быть изменена только путём обновления переменных окружения и перезапуска сервиса.

Update 2026-10: primary human/agent path is ado-cli login (see FR-8); the ENV-only statement above applies to the MCP server runtime.

---

## 4. Функциональные требования (FR) ✅

Каждое требование включает краткое описание и критерии приёмки.

FR-1: Create Work Item
- Описание: Инструмент `create_work_item` принимает `project`, `type` (любой тип WI), обязательные поля (минимум `System.Title`) и опциональные поля/relations/comments.
- Реализация: формируется `application/json-patch+json` документ и отправляется POST `/workitems/{type}?api-version=<version>`.
- Приёмка: возвращается объект с новыми `id`, `url` и набором полей, подтверждённым сервером Azure.

FR-2: Get Work Item
- Описание: `get_work_item` возвращает WI по `id` (или списку `ids`) с опциональными параметрами `fields` и `$expand` (Relations/Links/All/Comments).
- Реализация: GET `/workitems/{id}?fields=...&$expand=...`.
- Приёмка: возвращены поля и, при запросе `$expand`, relations и комментарии.

FR-3: Update Work Item
- Описание: `update_work_item` принимает JSON Patch (операции add/replace/remove/test и т.д.) и опционально параметр `expected_rev` (число). Если указан `expected_rev`, агент включит в JSON Patch `test` op на `/rev` для optimistic concurrency; при несовпадении вернётся ошибка `Conflict` (409). Если `expected_rev` не указан, PATCH выполняется без предварительного теста (как обычный PATCH), но рекомендуется передавать `expected_rev`, чтобы избежать потерянных обновлений.
- Реализация: PATCH `/workitems/{id}?api-version=...` с Content-Type `application/json-patch+json` и, при наличии `expected_rev`, первой операцией `test { "op":"test","path":"/rev","value":<expected_rev> }`.
- Приёмка: при успешном применении возвращается обновлённый WI с увеличенной ревизией; при несовпадении rev — возвращается `Conflict` с пояснением и текущей ревизией.

FR-4: Search Work Items
- Описание: `search_work_items` поддерживает WIQL и упрощённые фильтры; возвращает список id; опция `details=true` инициирует batch GET `/workitems?ids=...` для получения полных записей.
- Реализация: POST `/wit/wiql` → получение id → batch GET `/workitems?ids=...` (детали).
- Ограничения: в первом релизе PAGINATION не реализована — по умолчанию применяется `limit=100` (может быть переопределён параметром запроса). Если количество найденных id превышает практический порог использования, клиенту следует выполнить дополнительные запросы вручную.
- Приёмка: WIQL запрос возвращает ожидаемые id (ограничен до `limit`); при `details=true` — корректно возвращаются поля для возвращённых id.

FR-5: Manage Work Item Links
- Описание: `manage_work_item_link` — инструмент для добавления и удаления связей между рабочими элементами (тип связи: `System.LinkTypes.Related`, `System.LinkTypes.Hierarchy-Forward`/`Hierarchy-Reverse`, `System.LinkTypes.Dependency-Forward` и др.). Поддерживает операции `add` и `remove`.
- Входные параметры (MVP): `action` ("add" | "remove"), `source_id` (ид рабочего элемента, который изменяется), `target` (ид рабочего элемента-цели), `link_type` (опционально, по умолчанию `System.LinkTypes.Related`), `attributes` (опционально).
- Реализация: для `add` формируется JSON Patch op `add` на `/relations/-` с `{ "rel": <link_type>, "url": <target_url>, "attributes": ... }` и вызывается `update_work_item`; для `remove` выполняется `get_work_item` для поиска индекса существующей связи и `remove` op по `/relations/{index}`.
- Приёмка:
  - Успешный `add` возвращает обновлённый список связей в результате.
  - Успешный `remove` удаляет связь и возвращает обновлённый список связей.
  - Негативные случаи: удаление несуществующей связи возвращает понятную ошибку `relation not found`.
  - Схемы входа и выхода добавлены в `docs/schemas/manage_work_item_link.schema.json` и `manage_work_item_link_result.schema.json`.

FR-5: Управленческие команды и обнаружение проектов
- Описание: Конфигурация задаётся только через ENV (см. секцию 3.1); MCP предоставляет инструменты только для чтения/инспекции конфигурации и динамического обнаружения ресурсов.
- Команды:
  - `get_status` — возвращает текущую конфигурацию (без секретов) и состояние подключения к Azure DevOps.
  - `list_projects` — динамически запрашивает и возвращает список проектов, доступных для учётных данных из `ADO_TOKEN`.
- Проект по-умолчанию: если `project` не передан в инструменте, MCP использует в порядке приоритетов: 1) `ADO_DEFAULT_PROJECT` (если задан), 2) если доступен ровно один проект для токена — используется он, 3) иначе команда вернёт ошибку, требующую явного указания `project`.
- Примечание: команды `set_instance` / `set_token` **не поддерживаются** — изменение конфигурации требует обновления ENV и перезапуска сервиса.

FR-6: StdIO / rmcp Contract
- Описание: все команды регистрируются как rmcp tools с явными JSON-схемами входа/выхода (schemars).
- Приёмка: схемы валидируются, клиентская совместимость сохранена.

FR-7: Ошибки и retry
- Транзитивные ошибки (timeouts/5xx) — retry с экспоненциальным бэкоффом (макс 3 попытки).
- Клиентские ошибки (4xx) — не ретраить; возвращать структурированную ошибку с кодом и сообщением.

FR-8: Первичная авторизация (login)
- Описание: CLI-команда `ado-cli login` сохраняет PAT и URL сервера в пользовательский
  конфиг `~/.config/rusty_azure_devops/config.yaml` с правами 0600. Значения берутся из
  флагов (`--base-url`, `--token`, `--token-stdin`), затем env (`ADO_BASE_URL`, `ADO_TOKEN`),
  затем интерактивных запросов (только при TTY, ввод PAT скрыт). Перед сохранением
  выполняется проверка через `list_projects` (отключается `--no-verify`).
- Реализация: `ado-cli login [--base-url --token|--token-stdin --organization --project
  --api-version --ca-cert --force --no-verify --json]`, `ado-cli logout [--json]`.
- Приёмка: без TTY и без флагов/env команда завершается быстро с `missing_value`
  (код 1), а не блокируется на вводе; перезапись существующего конфига требует
  `--force` (иначе `confirmation_required`, код 3); при `--json` ошибки возвращаются
  в stderr как `{"ok":false,"reason","detail","hint"}`, stdout остаётся пустым; токен
  не выводится ни в одном режиме.

---

## 5. Нефункциональные требования (NFR) ⚙️

NFR-1: Производительность
- Цели: P50 < 300ms для простых GET/CREATE; P95 < 1s в нормальных условиях локальной сети.

NFR-2: Масштабируемость
- Ожидаемая нагрузка: порядка 0–100 RPS; архитектура должна позволять масштабировать azclient если потребуется.

NFR-3: Надёжность
- Цель availability: 99.5% в эксплуатационной среде при обычной нагрузке.

NFR-4: Расширяемость
- Интерфейс `AzureDevOpsClient` реализован как Rust trait; добавление новых backend‑драйверов возможно без изменений в `rusty_azure_devops`.

NFR-5: Наблюдаемость и логирование
- Структурированное логирование: обязателен `tracing`-совместимый формат с уровнями `info`, `warn`, `debug`, `trace`. Формат логов — JSON-совместимый с полями `timestamp`, `level`, `target`, `message`, `context`.
- Метрики: Prometheus/metrics не требуются в первом релизе; дополнительные метрики могут быть добавлены позже по запросу.
- Трассировка: OpenTelemetry — опционально и не обязательна для MVP. Логи не должны содержать секретов (API токены, ключи, полномочия).

NFR-6: Безопасность
- Аутентификация обязательна: PAT должен быть предоставлен через ENV `ADO_TOKEN` и `ADO_BASE_URL` (см. раздел 3.1). Без действительных значений этих переменных сервис не запускается.
- Секреты хранятся только в переменных окружения; **не логировать** секретные значения. Логи должны быть безопасны и допускать санитаризацию/маскирование секретов при необходимости.
- Подготовить инструкции по минимальным правам PAT (scope) и ротации токенов.

---

## 6. Архитектура и дизайн 🔧

### Компоненты
1. `rusty_azure_devops` — rmcp-based сервер: регистрирует инструменты, обрабатывает stdio transport (stdin/stdout) через `rmcp`, выполняет валидацию схем, управление таймаутами, маршрутизацию инструментов и lifecycle запросов/ответов.
2. `azclient` — `AzureDevOpsClient` trait и default тонкая реализация поверх `azure_devops_rust_api` (async). Инкапсулирует: API-versioning, retry/backoff с jitter, обработку rate-limit (429/`Retry-After`) и предоставляет высокоуровневые операции (create/get/update/search WI). Note: the retry/backoff utility is implemented inside `azure_devops_client::retry` (no separate crate for retry utilities is used).
3. `config` — ENV-only конфиг-лоадер и логика обнаружения проектов (`list_projects`), включая правило выбора `ADO_DEFAULT_PROJECT` или автоматическое использование единственного доступного проекта.
4. `logging` — централизованная подсистема структурированного логирования (`tracing`, JSON-compatible) с фильтрацией/санитизацией секретов и поддержкой уровней `info|warn|debug|trace`.

### Dataflow
Client → stdio/rmcp → rusty_azure_devops.tool → azclient (HTTP) → Azure DevOps → azclient → rusty_azure_devops → client (JSON).

### Design Decisions
- Использовать `rmcp` crate для реализации stdio transport и серверной логики (подробности см. ниже: релевантные фичи `rmcp`).
- `AzureDevOpsClient` как trait для легко тестируемых mock/fake реализаций.
- Retry/backoff встроен в `azclient`.
- Versioning конфигурации `api-version` — параметризуемая настройка.

### Рекомендуемые crates
- rmcp (MCP protocol, schema validation, stdio transport) — получать актуальную информацию через Context7 (см. примечание ниже).
- tokio (async runtime)
- reqwest (HTTP client) или hyper
- serde / serde_json (serialisation)
- schemars (JSON schema generation for tool contracts)
- tracing / opentelemetry / prometheus exporter
- thiserror / anyhow (errors)
- wiremock (tests)

---

## 7. Информация по `rmcp` (актуализирована через Context7) 🔎

Ключевые моменты (из Context7 `/websites/rs-rmcp`):
- `rmcp` предоставляет удобные примеры для создания сервисов и транспортов по stdio (stdin/stdout) и TCP.
- Поддерживает регистрацию инструментов и автоматическую проверку схем (schemars) для входа/выхода.
- Имеются готовые транспортные реализации (stdio, TCP, SSE client) и удобные макросы/методы для работы с peer (list_tools, call_tool и пр.).
- Рекомендуется использовать `serve_directly`/`serve_directly_with_ct` и Transport from Standard IO примеры при создании stdio сервера.

Ссылка: https://docs.rs/rmcp/latest/

---

## 8. StdIO инструменты (Tool Contracts) — схемы (описательно)

Каждый инструмент должен иметь JSON-схему входа и выхода (schemars) и быть описан в `docs/schemas/`.

Примеры (описательно):

`create_work_item` — вход:
```json
{ "project": "string", "type": "string", "fields": {"System.Title": "...", "Custom.Foo":"..."}, "relations": [...], "comments": [ { "author": "...", "text": "..." } ] }
```
выход:
```json
{ "id": 123, "url": "https://...", "fields": {...} }
```

Комментарии: MCP читает и пишет комментарии через Work Item Comments API (предпочтительный способ). Поведение:
- При `create_work_item` если `comments` переданы — MCP сначала создаёт WI, затем вызывает Comments API для добавления каждого комментария (POST `/wit/workItems/{id}/comments`).
- `get_work_item` при `$expand=Comments` возвращает комментарии из Comments API (включая автора, текст и timestamp).
- Для простоты и обратной совместимости MCP также поддерживает запись кратких комментариев через JSON Patch (добавление в поле `System.History`), но рекомендуется использовать Comments API.
`get_work_item` — вход:
```json
{ "id": 123, "project": "optional", "fields": ["System.Title","System.AssignedTo"], "expand": "All|Relations|None" }
```
выход: рабочий объект WorkItem (id, rev, fields, relations, _links, comments)

`update_work_item` — вход:
```json
{ "id": 123, "patch": [ {"op":"add","path":"/fields/System.Tags","value":"foo"} ] }
```
выход: обновлённый WorkItem

`search_work_items` — вход:
```json
{ "project": "optional", "wiql": "SELECT [System.Id] FROM WorkItems WHERE ...", "details": true }
```
выход: `{ "ids": [1,2,3], "total": 3 }` (при details=true — добавлять подробные записи)

---

## 9. Обработка ошибок и политика повтора (детали) ⚠️

- Транзитивные ошибки (timeouts, сетевые ошибки, 5xx): retry policy — экспоненциальный бэк‑офф с jitter (например 100ms, 300ms, 900ms), max 3 попытки; логировать попытки и итоговое состояние.
- Rate limiting (429): если Azure возвращает 429 и заголовок `Retry-After`, MCP обязан уважать `Retry-After` и повторить запрос после указанного времени, при необходимости применив общий retry logic с backoff.
- Конфликты ревизий: вернуть ошибку с кодом `Conflict` и пояснением (текущая ревизия != ожидаемая), дать рекомендации — выполнить `get_work_item` и повторить.
- Валидация входных данных: на уровне MCP инструментов — возвращать 400 с описанием поля/ошибки.

---

## 10. План тестирования и приёмки ✅

### Unit tests
- Парсинг входных сообщений, валидация схем, helper-функции (patch generation) — все покрываются unit-тестами.

### Integration tests
- Тестировать `azclient` против `wiremock` или аналогичного mock‑сервера с зафиксированными ответами Azure.
- Тестировать retry/timeout/404/5xx поведения.

### End-to-end tests
- E2E против локального Azure DevOps Server (если доступен) или тесты с записью/воспроизведением реальных ответов (record/replay fixtures).

### Contract tests (stdio)
- Проверять соответствие схем: корректные и негативные случаи; таймауты транспорта.

### Acceptance criteria по основным FR
- Create: создаёт WI и возвращает id/url; поля совпадают.
- Get: возвращает fields и relations при `$expand`.
- Update: применяет patch; rev увеличивается; на невыполнении теста возвращается Conflict.
- Search: WIQL возвращает ожидаемые id; details → batch GET возвращает корректные поля.

### CI
- GitHub Actions: lint, clippy, cargo test (unit + integration), validate schemars, run contract tests.

---

## 11. Риски и ограничения ⚠️

- Версионирование Azure REST API: разные on‑prem инсталляции могут поддерживать разные preview-версии; оставить параметр `api-version` настраиваемым.
- Отсутствие attachments и кеша в первом релизе ограничивает рабочие сценарии некоторыми пользователями.
- Trusted environment assumption: если MCP развернуто в менее доверенной сети, необходимо добавить auth/authorization и шифрование.

---

## 12. Открытые вопросы (требуют подтверждения) ❓

1. Требуется ли поддержка специальных полей/шаблонов WI по-умолчанию (выпилить/включить набор стандартных полей)?
2. Нужна ли поддержка массовых операций (bulk create/update)?
3. Уровень логирования по умолчанию и требования к retention для метрик/логов.

---

## 13. Acceptance checklist (короткий) ✅

- [ ] Create WI: id+url и поля корректно возвращаются.
- [ ] Get WI: fields+relations+comments доступны при `$expand`.
- [ ] Update WI: patch применяется и rev инкрементируется; конфликты корректно обрабатываются.
- [ ] Search WI: WIQL → ids; details → корректные записи.
- [ ] StdIO tools: схемы валидируются и соответствуют контракту.
- [x] Retry & error handling: покрыты тестами.

---

## 14. Приложения и ссылки 🔗

- Azure DevOps REST API: https://learn.microsoft.com/en-us/rest/api/azure/devops/
- Работа с WI (Create/Get/Update/WIQL): https://learn.microsoft.com/en-us/rest/api/azure/devops/wit/
- rmcp crate (актуализировано через Context7): https://docs.rs/rmcp/latest/
- Reference implementation (study): https://github.com/like-a-freedom/rusty-intervals-mcp
- Implementation Plan: ./IMPLEMENTATION_PLAN.md

---

## 15. Рекомендация: переиспользовать `azure_devops_rust_api` (реализация клиента)

Коротко: вместо написания низкоуровневого HTTP‑клиента вручную рекомендуется переиспользовать официальный crate `azure_devops_rust_api` (v0.32.0) — он автогенерируется из OpenAPI‑спецификации Azure DevOps и уже содержит модуль `wit` (Work Item Tracking), WIQL/search и сопутствующие клиенты.

Почему это безопасно и выгодно:
- Покрывает множество эндпоинтов (включая Work Item Tracking и Search).
- Поддерживает аутентификацию через `azure_identity` и PAT (`ADO_TOKEN`), уже протестирован и поддерживается Microsoft.
- Поддерживает feature‑флаги; можно подключить только нужные модули (например `wit`, `work`, `search`) чтобы уменьшить размер компиляции.

Как интегрировать в `rusty_azure_devops`:
1. Добавить зависимость в `Cargo.toml` (фиксировать минимальную проверенную версию):

```toml
azure_devops_rust_api = { version = "0.32.0", features = ["wit","work","search"] }
```

2. Реализовать `AzureDevOpsClient` trait (описан в разделе NFR/архитектуре) через thin wrapper над `azure_devops_rust_api`:
- `create_work_item` → вызов соответствующего метода в `wit` (формирование JSON Patch, POST)
- `get_work_item` → `wit::work_items_client().get(...)` с нужными query params (`$expand`, `fields`)
- `update_work_item` → `wit::work_items_client().update(...)` (JSON Patch)
- `search_work_items` → `wit::wiql_client().query_by_wiql(...)` и затем batch GET при `details=true`

3. Инкапсулировать логику retry/backoff, конвертацию ошибок и обработку конфликтов (`rev`/`test`) в wrapper, чтобы интерфейс MCP оставался предсказуемым и простым для тестирования.

4. Тестирование: написать integration tests, использующие `azure_devops_rust_api` в режиме mock (wiremock) или run/record реальных запросов; добавить примеры в `examples/`.

Примечание по аутентификации:
- `azure_devops_rust_api` поддерживает несколько подходов (аз CLI, `azure_identity`, PAT). Для on‑prem возможно использовать PAT через `ADO_TOKEN` или конфиг в `.env`.

Ссылки:
- Crates.io: https://crates.io/crates/azure_devops_rust_api
- Docs.rs: https://docs.rs/azure_devops_rust_api/latest/azure_devops_rust_api/
- Repository: https://github.com/microsoft/azure-devops-rust-api

---

*Конец спецификации.*

Если подтвердите — я добавлю JSON-схемы для инструментов в `docs/schemas/` и подготовлю PR с тестовой матрицей и заготовками тестов (unit/integration/contract).