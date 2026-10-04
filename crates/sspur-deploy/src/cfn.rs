use crate::iam::{fn_id, statements, table_id};
use crate::{pascal, Service};
use serde_json::{json, Map, Value};

fn attr_type(t: &sspur_check::Type, svc: &Service) -> &'static str {
    match t {
        sspur_check::Type::Con(n, _) if n == "Int" => "N",
        sspur_check::Type::Con(n, _) => match svc.layouts.newtypes.get(n) {
            Some(inner) => attr_type(inner, svc),
            None => "S",
        },
        _ => "S",
    }
}

pub fn route_id(method: &str, path: &str) -> String {
    format!("Route{}{}", pascal(method), pascal(&path.replace(['{', '}'], "")))
}

pub fn template(svc: &Service) -> Value {
    let mut res = Map::new();
    for s in &svc.stores {
        res.insert(
            table_id(&s.name),
            json!({
                "Type": "AWS::DynamoDB::Table",
                "DeletionPolicy": "Retain",
                "UpdateReplacePolicy": "Retain",
                "Properties": {
                    "BillingMode": "PAY_PER_REQUEST",
                    "AttributeDefinitions": [{"AttributeName": "pk", "AttributeType": attr_type(&s.key, svc)}],
                    "KeySchema": [{"AttributeName": "pk", "KeyType": "HASH"}],
                    "PointInTimeRecoverySpecification": {"PointInTimeRecoveryEnabled": true},
                    "SSESpecification": {"SSEEnabled": true},
                    "Tags": [{"Key": "sspur:store", "Value": s.name}, {"Key": "sspur:value", "Value": s.val.to_string()}]
                }
            }),
        );
    }
    res.insert("Api".into(), json!({"Type": "AWS::ApiGatewayV2::Api", "Properties": {"Name": {"Fn::Sub": "${AWS::StackName}"}, "ProtocolType": "HTTP", "Description": format!("sspur svc {} #{}", svc.name, svc.hash)}}));
    res.insert(
        "ApiStage".into(),
        json!({"Type": "AWS::ApiGatewayV2::Stage", "Properties": {"ApiId": {"Ref": "Api"}, "StageName": "$default", "AutoDeploy": true, "DefaultRouteSettings": {"ThrottlingBurstLimit": 100, "ThrottlingRateLimit": 50}}}),
    );
    let backfill_names: Vec<String> = svc.backfills.iter().map(|h| h.name.clone()).collect();
    for h in svc.handlers.iter().chain(&svc.backfills) {
        let id = fn_id(&h.name);
        let backfill = backfill_names.contains(&h.name);
        let mut env = Map::new();
        if let Some(store) = h.name.strip_prefix("backfill_").filter(|_| backfill) {
            env.insert("SSPUR_BACKFILL".into(), json!(store));
        } else {
            env.insert("SSPUR_HANDLER".into(), json!(h.name));
        }
        for s in h.stores() {
            env.insert(format!("SSPUR_TABLE_{s}"), json!({"Ref": table_id(&s)}));
        }
        res.insert(format!("{id}Logs"), json!({"Type": "AWS::Logs::LogGroup", "Properties": {"RetentionInDays": 30}}));
        res.insert(
            format!("{id}Role"),
            json!({
                "Type": "AWS::IAM::Role",
                "Properties": {
                    "Description": format!("effects of {}: {}", h.name, if h.row.is_empty() { "pure".to_string() } else { h.row.join(", ") }),
                    "AssumeRolePolicyDocument": {
                        "Version": "2012-10-17",
                        "Statement": [{"Effect": "Allow", "Principal": {"Service": "lambda.amazonaws.com"}, "Action": "sts:AssumeRole", "Condition": {"StringEquals": {"aws:SourceAccount": {"Ref": "AWS::AccountId"}}}}]
                    },
                    "Policies": [{"PolicyName": "sspur-effects", "PolicyDocument": {"Version": "2012-10-17", "Statement": statements(h)}}]
                }
            }),
        );
        res.insert(
            format!("{id}Function"),
            json!({
                "Type": "AWS::Lambda::Function",
                "Properties": {
                    "Runtime": "provided.al2023",
                    "Handler": "bootstrap",
                    "Architectures": ["arm64"],
                    "MemorySize": 256,
                    "Timeout": if backfill { 300 } else { 10 },
                    "Code": {"S3Bucket": {"Ref": "ArtifactBucket"}, "S3Key": {"Ref": "ArtifactKey"}},
                    "Role": {"Fn::GetAtt": [format!("{id}Role"), "Arn"]},
                    "LoggingConfig": {"LogGroup": {"Ref": format!("{id}Logs")}, "LogFormat": "Text"},
                    "Environment": {"Variables": env}
                }
            }),
        );
        if backfill {
            continue;
        }
        rollout(&mut res, svc, &id);
        res.insert(
            format!("{id}Integration"),
            json!({"Type": "AWS::ApiGatewayV2::Integration", "Properties": {"ApiId": {"Ref": "Api"}, "IntegrationType": "AWS_PROXY", "IntegrationUri": {"Ref": format!("{id}Alias")}, "PayloadFormatVersion": "2.0"}}),
        );
    }
    let fns: Vec<String> = svc.handlers.iter().map(|h| fn_id(&h.name)).collect();
    res.insert("CodeDeployApp".into(), json!({"Type": "AWS::CodeDeploy::Application", "Properties": {"ComputePlatform": "Lambda"}}));
    let mut lambda_res: Vec<Value> = fns.iter().map(|f| json!({"Fn::GetAtt": [format!("{f}Function"), "Arn"]})).collect();
    lambda_res.extend(fns.iter().map(|f| json!({"Fn::Sub": format!("${{{f}Function.Arn}}:{ALIAS}")})));
    let alarms: Vec<Value> = fns.iter().flat_map(|f| [json!({"Fn::GetAtt": [format!("{f}ErrorsAlarm"), "Arn"]}), json!({"Fn::GetAtt": [format!("{f}FailuresAlarm"), "Arn"]})]).collect();
    res.insert(
        "CodeDeployRole".into(),
        json!({
            "Type": "AWS::IAM::Role",
            "Properties": {
                "Description": "shifts traffic between function versions of this service and nothing else",
                "AssumeRolePolicyDocument": {
                    "Version": "2012-10-17",
                    "Statement": [{"Effect": "Allow", "Principal": {"Service": "codedeploy.amazonaws.com"}, "Action": "sts:AssumeRole", "Condition": {"StringEquals": {"aws:SourceAccount": {"Ref": "AWS::AccountId"}}}}]
                },
                "Policies": [{"PolicyName": "sspur-rollout", "PolicyDocument": {"Version": "2012-10-17", "Statement": [
                    {"Sid": "Aliases", "Effect": "Allow", "Action": ["lambda:GetAlias", "lambda:GetFunction", "lambda:GetProvisionedConcurrencyConfig", "lambda:UpdateAlias"], "Resource": lambda_res},
                    {"Sid": "Alarms", "Effect": "Allow", "Action": ["cloudwatch:DescribeAlarms"], "Resource": alarms}
                ]}}]
            }
        }),
    );
    for r in &svc.routes {
        let id = fn_id(&r.handler);
        let rid = route_id(&r.method, &r.path);
        let arn_path: String = r.path.split('/').map(|s| if s.starts_with('{') { "*" } else { s }).collect::<Vec<_>>().join("/");
        res.insert(rid.clone(), json!({"Type": "AWS::ApiGatewayV2::Route", "Properties": {"ApiId": {"Ref": "Api"}, "RouteKey": r.key(), "Target": {"Fn::Join": ["/", ["integrations", {"Ref": format!("{id}Integration")}]]}}}));
        res.insert(
            format!("{rid}Permission"),
            json!({
                "Type": "AWS::Lambda::Permission",
                "Properties": {
                    "Action": "lambda:InvokeFunction",
                    "FunctionName": {"Ref": format!("{id}Alias")},
                    "Principal": "apigateway.amazonaws.com",
                    "SourceArn": {"Fn::Sub": format!("arn:${{AWS::Partition}}:execute-api:${{AWS::Region}}:${{AWS::AccountId}}:${{Api}}/*/{}{}", r.method.to_uppercase(), arn_path)}
                }
            }),
        );
    }
    json!({
        "AWSTemplateFormatVersion": "2010-09-09",
        "Description": format!("sspur svc {} #{} (generated by sspur deploy plan)", svc.name, svc.hash),
        "Parameters": {
            "ArtifactBucket": {"Type": "String", "Description": "S3 bucket holding bootstrap.zip"},
            "ArtifactKey": {"Type": "String", "Description": "S3 key of bootstrap.zip"},
            "TrafficShift": {"Type": "String", "Default": DEFAULT_SHIFT, "AllowedValues": SHIFTS, "Description": "CodeDeploy traffic shift from the live version to this one"}
        },
        "Resources": res,
        "Outputs": outputs(svc)
    })
}

pub const ALIAS: &str = "live";
pub const DEFAULT_SHIFT: &str = "CodeDeployDefault.LambdaCanary10Percent5Minutes";
const SHIFTS: [&str; 4] = [DEFAULT_SHIFT, "CodeDeployDefault.LambdaLinear10PercentEvery1Minute", "CodeDeployDefault.LambdaCanary10Percent30Minutes", "CodeDeployDefault.LambdaAllAtOnce"];

fn outputs(svc: &Service) -> Value {
    let mut o = Map::new();
    o.insert("ApiUrl".into(), json!({"Value": {"Fn::GetAtt": ["Api", "ApiEndpoint"]}}));
    o.insert("Version".into(), json!({"Value": svc.hash}));
    for b in &svc.backfills {
        o.insert(format!("{}Function", fn_id(&b.name)), json!({"Value": {"Ref": format!("{}Function", fn_id(&b.name))}}));
    }
    Value::Object(o)
}

fn alarm(desc: String, ns: Value, metric: Value, dims: Option<Value>) -> Value {
    let mut p = json!({
        "AlarmDescription": desc,
        "Namespace": ns, "MetricName": metric, "Statistic": "Sum", "Period": 60, "EvaluationPeriods": 1, "Threshold": 1,
        "ComparisonOperator": "GreaterThanOrEqualToThreshold", "TreatMissingData": "notBreaching"
    });
    if let Some(d) = dims {
        p["Dimensions"] = d;
    }
    json!({"Type": "AWS::CloudWatch::Alarm", "Properties": p})
}

fn rollout(res: &mut Map<String, Value>, svc: &Service, id: &str) {
    let version = format!("{id}Version{}", svc.hash);
    res.insert(
        version.clone(),
        json!({"Type": "AWS::Lambda::Version", "DeletionPolicy": "Retain", "UpdateReplacePolicy": "Retain", "Properties": {"FunctionName": {"Ref": format!("{id}Function")}, "Description": format!("sspur #{}", svc.hash)}}),
    );
    res.insert(
        format!("{id}Alias"),
        json!({
            "Type": "AWS::Lambda::Alias",
            "UpdatePolicy": {"CodeDeployLambdaAliasUpdate": {"ApplicationName": {"Ref": "CodeDeployApp"}, "DeploymentGroupName": {"Ref": format!("{id}DeployGroup")}}},
            "Properties": {"FunctionName": {"Ref": format!("{id}Function")}, "FunctionVersion": {"Fn::GetAtt": [version, "Version"]}, "Name": ALIAS}
        }),
    );
    let dims = json!([{"Name": "FunctionName", "Value": {"Ref": format!("{id}Function")}}, {"Name": "Resource", "Value": {"Fn::Sub": format!("${{{id}Function}}:{ALIAS}")}}]);
    res.insert(format!("{id}ErrorsAlarm"), alarm(format!("{id}:{ALIAS} invocation errors; stops a traffic shift"), json!("AWS/Lambda"), json!("Errors"), Some(dims)));
    let ns = format!("sspur/{}", svc.name);
    res.insert(
        format!("{id}Failures"),
        json!({
            "Type": "AWS::Logs::MetricFilter",
            "Properties": {
                "LogGroupName": {"Ref": format!("{id}Logs")},
                "FilterPattern": "[tag=REQ, id, status=5*, ms]",
                "MetricTransformations": [{"MetricNamespace": ns, "MetricName": format!("{id}5xx"), "MetricValue": "1", "DefaultValue": 0}]
            }
        }),
    );
    res.insert(format!("{id}FailuresAlarm"), alarm(format!("{id} answered 5xx; stops a traffic shift"), json!(ns), json!(format!("{id}5xx")), None));
    res.insert(
        format!("{id}DeployGroup"),
        json!({
            "Type": "AWS::CodeDeploy::DeploymentGroup",
            "Properties": {
                "ApplicationName": {"Ref": "CodeDeployApp"},
                "ServiceRoleArn": {"Fn::GetAtt": ["CodeDeployRole", "Arn"]},
                "DeploymentConfigName": {"Ref": "TrafficShift"},
                "DeploymentStyle": {"DeploymentType": "BLUE_GREEN", "DeploymentOption": "WITH_TRAFFIC_CONTROL"},
                "AutoRollbackConfiguration": {"Enabled": true, "Events": ["DEPLOYMENT_FAILURE", "DEPLOYMENT_STOP_ON_ALARM", "DEPLOYMENT_STOP_ON_REQUEST"]},
                "AlarmConfiguration": {"Enabled": true, "Alarms": [{"Name": {"Ref": format!("{id}ErrorsAlarm")}}, {"Name": {"Ref": format!("{id}FailuresAlarm")}}]}
            }
        }),
    );
}

const BUILD: &str = r#"#!/bin/sh
# Builds bootstrap.zip for provided.al2023 on arm64 inside an Amazon Linux 2023 container. Needs docker; touches no AWS account.
set -eu
cd "$(dirname "$0")"
docker run --rm --platform linux/arm64 -v "$PWD":/w -w /w public.ecr.aws/amazonlinux/amazonlinux:2023 sh -c '
  dnf install -y --allowerasing clang libcurl-devel zip findutils >/dev/null
  clang -O2 -w -o bootstrap bootstrap.c -lcurl -lpthread -lm
  rm -rf lib && mkdir lib
  ldd bootstrap | awk "/=> \//{print \$3}" | grep -v -E "/(libc|libm|libpthread|libdl|librt|ld-linux)[.-]" | xargs -r -I{} cp -L {} lib/
  rm -f bootstrap.zip && zip -qr bootstrap.zip bootstrap lib'
echo "built bootstrap.zip"
"#;

pub fn build_script(_svc: &Service) -> String {
    BUILD.to_string()
}

pub fn deploy_script(svc: &Service) -> String {
    format!(
        r#"#!/bin/sh
# Deploys this plan with the caller's AWS credentials. sspur never runs it: deploying is an explicit operator decision.
set -eu
: "${{BUCKET:?set BUCKET to an S3 bucket for build artifacts}}"
STACK="${{STACK:-sspur-{name}}}"
KEY="sspur/{name}/{hash}.zip"
cd "$(dirname "$0")"
test -f bootstrap.zip || ./build.sh
aws s3 cp bootstrap.zip "s3://$BUCKET/$KEY"
aws s3 cp template.json "s3://$BUCKET/sspur/{name}/{hash}.template.json"
# CloudFormation hands each alias update to CodeDeploy, which shifts traffic by TRAFFIC_SHIFT and rolls back on the alarms.
aws cloudformation deploy --stack-name "$STACK" --template-file template.json --capabilities CAPABILITY_IAM --parameter-overrides ArtifactBucket="$BUCKET" ArtifactKey="$KEY" TrafficShift="${{TRAFFIC_SHIFT:-{shift}}}"
aws cloudformation describe-stacks --stack-name "$STACK" --query "Stacks[0].Outputs" --output table
"#,
        name = svc.name,
        hash = svc.hash,
        shift = DEFAULT_SHIFT
    )
}

pub fn rollback_script(svc: &Service) -> String {
    format!(
        r#"#!/bin/sh
# Rolls back with the caller's AWS credentials. sspur never runs it.
# During a traffic shift it stops the shift, and CodeDeploy points every alias back at the previous version.
# After a finished shift, PREV=<hash> redeploys that version's retained artifacts all at once.
set -eu
STACK="${{STACK:-sspur-{name}}}"
APP=$(aws cloudformation describe-stack-resource --stack-name "$STACK" --logical-resource-id CodeDeployApp --query StackResourceDetail.PhysicalResourceId --output text)
ACTIVE=$(aws deploy list-deployments --application-name "$APP" --include-only-statuses Created Queued InProgress Ready --query deployments --output text)
if [ -n "$ACTIVE" ] && [ "$ACTIVE" != "None" ]; then
  for d in $ACTIVE; do aws deploy stop-deployment --deployment-id "$d" --auto-rollback-enabled; done
  echo "stopped $ACTIVE; aliases return to the previous version"
  exit 0
fi
: "${{PREV:?no shift in progress; set PREV to the version hash to roll back to (stack output Version)}}"
: "${{BUCKET:?set BUCKET to the artifact bucket}}"
cd "$(dirname "$0")"
aws s3 cp "s3://$BUCKET/sspur/{name}/$PREV.template.json" "prev-$PREV.template.json"
aws cloudformation deploy --stack-name "$STACK" --template-file "prev-$PREV.template.json" --capabilities CAPABILITY_IAM --parameter-overrides ArtifactBucket="$BUCKET" ArtifactKey="sspur/{name}/$PREV.zip" TrafficShift=CodeDeployDefault.LambdaAllAtOnce
"#,
        name = svc.name
    )
}

pub fn backfill_script(svc: &Service) -> String {
    let mut s = format!(
        r#"#!/bin/sh
# Backfills migrated stores page by page with the caller's AWS credentials. sspur never runs it.
# Run it once the traffic shift to #{hash} has finished. It is idempotent and safe under live traffic:
# each write is conditional on the item being unchanged since it was scanned.
set -eu
STACK="${{STACK:-sspur-{name}}}"
cd "$(dirname "$0")"
"#,
        name = svc.name,
        hash = svc.hash
    );
    for b in &svc.backfills {
        let id = format!("{}Function", fn_id(&b.name));
        s.push_str(&format!(
            r#"FN=$(aws cloudformation describe-stack-resource --stack-name "$STACK" --logical-resource-id {id} --query StackResourceDetail.PhysicalResourceId --output text)
START=null
while :; do
  printf '{{"limit":100,"start":%s}}' "$START" > backfill-payload.json
  aws lambda invoke --function-name "$FN" --cli-binary-format raw-in-base64-out --payload file://backfill-payload.json backfill-out.json >/dev/null
  cat backfill-out.json; echo
  START=$(python3 -c 'import json; print(json.dumps(json.load(open("backfill-out.json")).get("next")))')
  [ "$START" = "null" ] && break
done
"#
        ));
    }
    s
}
