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
    for h in &svc.handlers {
        let id = fn_id(&h.name);
        let mut env = Map::new();
        env.insert("SSPUR_HANDLER".into(), json!(h.name));
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
                    "Timeout": 10,
                    "Code": {"S3Bucket": {"Ref": "ArtifactBucket"}, "S3Key": {"Ref": "ArtifactKey"}},
                    "Role": {"Fn::GetAtt": [format!("{id}Role"), "Arn"]},
                    "LoggingConfig": {"LogGroup": {"Ref": format!("{id}Logs")}, "LogFormat": "Text"},
                    "Environment": {"Variables": env}
                }
            }),
        );
        res.insert(
            format!("{id}Integration"),
            json!({"Type": "AWS::ApiGatewayV2::Integration", "Properties": {"ApiId": {"Ref": "Api"}, "IntegrationType": "AWS_PROXY", "IntegrationUri": {"Fn::GetAtt": [format!("{id}Function"), "Arn"]}, "PayloadFormatVersion": "2.0"}}),
        );
    }
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
                    "FunctionName": {"Ref": format!("{id}Function")},
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
            "ArtifactKey": {"Type": "String", "Description": "S3 key of bootstrap.zip"}
        },
        "Resources": res,
        "Outputs": {"ApiUrl": {"Value": {"Fn::GetAtt": ["Api", "ApiEndpoint"]}}}
    })
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
aws cloudformation deploy --stack-name "$STACK" --template-file template.json --capabilities CAPABILITY_IAM --parameter-overrides ArtifactBucket="$BUCKET" ArtifactKey="$KEY"
aws cloudformation describe-stacks --stack-name "$STACK" --query "Stacks[0].Outputs" --output table
"#,
        name = svc.name,
        hash = svc.hash
    )
}
