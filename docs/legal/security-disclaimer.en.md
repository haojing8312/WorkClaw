# WorkClaw Security Disclaimer

Effective date: 2026-03-11

This document explains the security boundaries, inherent risks, user responsibilities, and limitations of liability associated with installing, configuring, integrating, and operating WorkClaw and its related components. You must read this document carefully before downloading, installing, compiling, deploying, configuring, connecting third-party services to, importing Skills into, or otherwise using WorkClaw.

By continuing to download, install, copy, compile, deploy, configure, integrate, access, or use WorkClaw, you acknowledge that you have read, understood, and accepted this disclaimer. If you do not agree with any part of this document, do not download, install, or use WorkClaw.

## 1. Scope

This disclaimer applies to the WorkClaw open-source repository, desktop application, command execution capabilities, browser automation capabilities, model integrations, MCP extensions, Skill installation and packaging flows, and any third-party models, plugins, scripts, services, credentials, working directories, or runtime environments connected by the user.

## 2. Product Capabilities and Inherent Risks

WorkClaw is desktop agent software with system-level operational capabilities. Depending on your configuration and authorization choices, WorkClaw may be able to:

- read, write, move, overwrite, or delete files
- execute local commands, scripts, build pipelines, or automation workflows
- access local working directories, session data, logs, and configuration files
- invoke browser automation, web access, search, and third-party APIs
- connect to external models, MCP services, Skill packages, and other extension components

These capabilities inherently involve security risk, including but not limited to data modification or deletion, exposure of sensitive information, unintended command execution, misuse of external services, third-party supply-chain risk, cost overruns, expanded privileges caused by unsafe configuration, and business, compliance, or reputational harm caused by automation.

## 3. User Security Responsibilities

You are solely responsible for the deployment model, runtime environment, permission boundaries, and external integrations used with WorkClaw, including but not limited to:

- carefully configuring working directories, file access scope, environment variables, secrets, and account credentials
- operating WorkClaw only on trusted devices, trusted networks, and appropriately controlled accounts
- retaining human review, approvals, or staged confirmation for high-risk actions
- independently validating impact before deletion, overwrite, bulk edit, publishing, messaging, syncing, or deployment actions
- maintaining appropriate backup, audit, access-control, logging, and recovery measures
- ensuring that all use complies with applicable law, contractual obligations, industry rules, and internal policies

If you disable approvals, broaden accessible paths, grant high-privilege tokens, allow unrestricted external input, or run automation directly in production environments, your risk exposure increases materially and remains your responsibility.

## 4. Risks from Third-Party Skills, Models, and Services

WorkClaw supports third-party Skills, model providers, MCP services, scripts, packages, and other extensions. Those third-party components are not automatically controlled, audited, warranted, or continuously verified by the WorkClaw project maintainers.

You are responsible for performing your own source and security review, including:

- reviewing the origin, code quality, and permission requirements of Skills, scripts, dependencies, and extension components
- verifying the billing model, data-handling policies, and service commitments of third-party model providers, proxy services, relay gateways, and API platforms
- assessing supply-chain, unauthorized access, data transfer, and compliance risks introduced by third-party services

The WorkClaw project maintainers provide no express or implied warranty for any loss caused by third-party Skills, third-party code, third-party models, third-party services, or any change, outage, vulnerability, or malicious behavior affecting them.

## 5. Risks from External Input and Automated Decisions

When WorkClaw processes content from websites, email, chat messages, documents, code repositories, or other external sources, it may be influenced by misleading input, malicious instructions, prompt injection, context poisoning, or forged information. Any execution advice, operational plan, command, code, or recommendation generated from external input must not be treated as inherently trustworthy.

You are responsible for independently reviewing:

- the authenticity, completeness, and authorization boundary of external content
- the correctness of generated commands, scripts, configurations, patches, and published content
- whether a workflow involves deletion, payment, messaging, public release, permission changes, or other irreversible actions

## 6. Data, Privacy, and Cost Responsibility

Although WorkClaw is designed around local control, any models, search providers, browser tooling, MCP services, or other third-party services you connect may involve data transfer, logging, caching, metering, billing, and compliance obligations. You are responsible for reviewing the privacy policy, service terms, data residency, pricing rules, and quota limits of those platforms.

The WorkClaw project maintainers are not responsible for:

- increased charges, token consumption, credits depletion, or account loss caused by misconfiguration, excessive invocation, incorrect routing, or third-party billing rules
- leakage, misuse, or compliance failures arising from sensitive data that you choose to input, mount, expose, or transmit
- service interruption, throttling, suspension, or policy changes imposed by third-party platforms

## 7. No Warranty

WorkClaw is provided on an "as is" and "as available" basis without warranties of any kind, express or implied, including without limitation warranties of merchantability, fitness for a particular purpose, stability, uninterrupted availability, accuracy, security, or suitability for any specific result.

The project maintainers do not warrant that:

- the software will satisfy your business objectives, compliance requirements, or internal security standards
- any version is free from vulnerabilities, defects, false positives, output errors, or compatibility issues
- any security restriction, approval mechanism, or permission control will cover every scenario or attack path

## 8. Limitation of Liability

To the maximum extent permitted by applicable law, the WorkClaw project maintainers, contributors, and rights holders shall not be liable for any direct, indirect, incidental, special, punitive, or consequential loss arising from use, inability to use, misuse, unsafe configuration, third-party integration, external input, Skill installation, command execution, automation behavior, or security incidents. This includes without limitation:

- data loss, file corruption, or system unavailability
- business interruption, lost revenue, or lost opportunity
- account issues, cost overruns, or credential exposure
- regulatory penalties, third-party claims, or reputational harm

Where applicable law does not allow the exclusion of certain liability, liability is limited only to the minimum extent required by law.

## 9. Condition of Use

You should use WorkClaw only if you are able to assess the relevant risks, control the authorization boundary, and accept the resulting consequences. If you use WorkClaw on behalf of a company, team, or other organization, you represent and warrant that you have the necessary authority to do so and that you are responsible for ensuring such use complies with internal policy and applicable law.

If you cannot accept the risk allocation and limitation terms stated in this document, do not download, install, copy, deploy, or use WorkClaw.

## 10. Updates

The WorkClaw project may update this document to reflect changes in product capability, runtime behavior, integration scope, or legal and compliance requirements. An updated version becomes effective on the date stated in the document. You are responsible for reviewing the latest version before upgrading, enabling new capabilities, or continuing to use WorkClaw.

## 11. Related Documents

- Security reporting and vulnerability disclosure: `SECURITY.md`
- User-facing security guidance: `docs/user-manual/08-security.md`

