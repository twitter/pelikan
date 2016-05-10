---
layout: default
title: Talks
permalink: /talks/
---
  <ul class="talk-list">
    {% for talk in site.talks %}
      <div class="page-col-wrapper">
        <div class="page-col">
          <a href="{{ site.baseurl }}{{ talk.cover }}">
          <img src="{{ site.baseurl }}{{ talk.cover }}" alt="cover image">
          </a>
        </div>
        <div class="page-double-col">
          <h2>
            <a class="talk-link" href="{{ talk.url | prepend: site.baseurl }}">{{ talk.title }}</a>
          </h2>
          <span class="talk-meta">{{ talk.date | date: "%b %Y" }}</span>
        </div>
      </div>
    {% endfor %}
  </ul>
